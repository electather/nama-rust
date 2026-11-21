// The fake ffprobe is a shell script. The Version is stored without a scan, so no background pass
// probes it.
#![cfg(unix)]

use std::{collections::BTreeSet, sync::Arc};

use tempfile::TempDir;

use super::{Asked, AskedSubtitle, Pick, PickError, Plans};
use crate::{
    db::Database,
    events::EventBus,
    ffmpeg::{Abilities, Ffmpeg},
    images::ImageStore,
    library::{
        Contents, Decided,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{
        Description, DeviceDecoding, PickedLimit, Plan, Planned, Preferred, PreviewFrames, Probing,
        RunningPlans, StartedPlan, SubtitlePreference, subtitles,
    },
    plugins::{ExtensionPoints, Registry},
    server_settings::{ServerUrl, Settings},
    users::{Caller, Device, Permissions, User, Users},
};

/// An ffprobe that finds H.264 video, AAC audio and an English `SubRip` subtitle in any file.
const FFPROBE: &str = r#"#!/bin/sh
echo '{"streams": [{"codec_name": "h264", "codec_type": "video"},
                   {"codec_name": "aac", "codec_type": "audio"},
                   {"codec_name": "subrip", "codec_type": "subtitle", "tags": {"language": "eng"}}],
       "format": {"format_name": "mov,mp4,m4a,3gp,3g2,mj2"}}'
"#;

/// The place of the audio Track in what [`FFPROBE`] finds.
const AUDIO: usize = 1;
/// The place of the subtitle Track in what [`FFPROBE`] finds.
const SUBTITLE: usize = 2;

/// Plans on a database in a temp folder, with Ripley's Device and a Matched Movie whose Version's
/// Tracks aren't probed yet.
struct Alien {
    plans: Plans,
    db: Arc<Database>,
    ripley: Caller,
    item: i64,
    version: i64,
    _folders: [TempDir; 2],
}

impl Alien {
    async fn new() -> Self {
        let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(348, "Alien")).await;
        let ffprobe = Ffmpeg {
            ffprobe: Ok(crate::ffmpeg::fake::script(data.path(), "ffprobe", FFPROBE)),
            ..Ffmpeg::default()
        };
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let probing = Probing::new(
            Arc::clone(&db),
            Arc::new(ffprobe),
            subtitles::Cache::new(data.path()),
            images.clone(),
        );
        let settings = Settings::new(Arc::clone(&db), data.path().join("transcodes"));
        let ffmpeg = Arc::new(Ffmpeg::default());
        let events = EventBus::new(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let running = RunningPlans::new(
            settings.playback(),
            Arc::clone(&ffmpeg),
            Contents::new(Arc::clone(&db)),
            probing.clone(),
            events,
        );
        let preview_frames = PreviewFrames::new(
            Arc::clone(&db),
            images,
            Arc::clone(&ffmpeg),
            running.clone(),
            probing.clone(),
        );
        let plans = Plans::new(
            probing,
            Abilities::new(ffmpeg),
            running,
            settings.playback(),
            preview_frames,
            Arc::clone(&db),
        );
        let ripley = ripley_on_the_nostromo(&db).await;
        Self {
            plans,
            db,
            ripley,
            item: alien.item,
            version: alien.version,
            _folders: [data, media],
        }
    }

    fn user(&self) -> i64 {
        self.ripley.user.id
    }

    /// The plan of the Movie, asking `subtitle` of subtitles and leaving the rest to the Server.
    async fn plan(&self, subtitle: AskedSubtitle) -> Plan {
        let asked = Asked {
            version: self.version,
            decoding: DeviceDecoding::default(),
            failed: BTreeSet::new(),
            audio: None,
            subtitle,
            limit: PickedLimit::Unpicked,
            start_ms: 0,
            client: std::net::Ipv4Addr::LOCALHOST.into(),
        };
        match self.plans.plan(&self.ripley, asked).await.unwrap() {
            Some(Planned::Started(StartedPlan { plan, .. }) | Planned::Unplayable { plan, .. }) => {
                plan
            }
            None => panic!("no such Version"),
        }
    }

    async fn keep(&self, pick: Pick) -> Result<(), PickError> {
        self.plans.keep_pick(self.user(), self.version, pick).await
    }

    /// The Subtitle Preference a plan of the Movie starts with for Ripley.
    async fn subtitle_preference(&self) -> Option<Preferred<SubtitlePreference>> {
        let preferences = &self.plans.preferences;
        preferences
            .subtitle(self.user(), self.version)
            .await
            .unwrap()
    }

    async fn record_unmatched(&self) {
        Contents::new(Arc::clone(&self.db))
            .record(vec![(self.item, Decided::Unmatched(None))])
            .await
            .unwrap();
    }
}

/// Ripley, a User stored in `db`, on her Device the Nostromo.
async fn ripley_on_the_nostromo(db: &Arc<Database>) -> Caller {
    let root = User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    };
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    let id = Users::new(Arc::clone(db), plugins.extension_points())
        .create(&root, "Ripley", "correct horse battery staple")
        .await
        .unwrap();
    Caller {
        user: User {
            id,
            name: "Ripley".to_owned(),
            permissions: Permissions::NEW_USER,
            created_at: 0,
        },
        device: Device {
            id: 1,
            name: "Nostromo".to_owned(),
            created_at: 0,
            last_seen_at: 0,
            authenticated_at: 0,
        },
    }
}

fn english() -> SubtitlePreference {
    SubtitlePreference::Like(Description {
        language: Some("eng".to_owned()),
        ..Description::default()
    })
}

fn kept_for_the_movie(preference: SubtitlePreference) -> Preferred<SubtitlePreference> {
    Preferred {
        preference,
        users_own: false,
        foreign_audio_only: false,
    }
}

#[tokio::test]
async fn a_version_without_tracks_is_probed_before_its_playback_plan_is_built() {
    let alien = Alien::new().await;

    let plan = alien.plan(AskedSubtitle::None).await;

    let codecs: Vec<&str> = plan
        .tracks
        .iter()
        .map(|track| track.codec.as_str())
        .collect();
    assert_eq!(codecs, ["h264", "aac"]);
}

#[tokio::test]
async fn a_plan_starts_with_the_subtitle_kept_for_the_movie_without_the_caller_reading_it() {
    let alien = Alien::new().await;
    let preferences = &alien.plans.preferences;
    let kept_for = preferences.kept_for(alien.version).await.unwrap().unwrap();
    (preferences.keep_subtitle(alien.user(), kept_for, english()))
        .await
        .unwrap();

    let plan = alien.plan(AskedSubtitle::Preferred).await;

    assert_eq!(plan.starting_subtitle, Some(SUBTITLE));
}

#[tokio::test]
async fn a_subtitle_pick_and_subtitles_turned_off_are_kept_for_the_movie() {
    let alien = Alien::new().await;

    alien.keep(Pick::Subtitle(Some(SUBTITLE))).await.unwrap();
    let picked = alien.subtitle_preference().await;
    alien.keep(Pick::Subtitle(None)).await.unwrap();
    let turned_off = alien.subtitle_preference().await;

    assert_eq!(picked, Some(kept_for_the_movie(english())));
    assert_eq!(
        turned_off,
        Some(kept_for_the_movie(SubtitlePreference::Off))
    );
}

#[tokio::test]
async fn a_pick_in_an_unmatched_items_version_is_accepted_and_kept_nowhere_whatever_it_names() {
    let alien = Alien::new().await;
    alien.record_unmatched().await;

    alien.keep(Pick::Subtitle(Some(AUDIO))).await.unwrap();
    alien.keep(Pick::Audio(99)).await.unwrap();

    assert_eq!(alien.subtitle_preference().await, None);
    let audio = alien.plans.preferences.audio(alien.user(), alien.version);
    assert_eq!(audio.await.unwrap(), None);
}

#[tokio::test]
async fn a_subtitle_pick_at_a_place_holding_audio_or_past_the_tracks_is_track_not_found() {
    let alien = Alien::new().await;

    let at_audio = alien.keep(Pick::Subtitle(Some(AUDIO))).await;
    let past_the_tracks = alien.keep(Pick::Subtitle(Some(3))).await;

    assert!(matches!(at_audio, Err(PickError::TrackNotFound)));
    assert!(matches!(past_the_tracks, Err(PickError::TrackNotFound)));
    assert_eq!(alien.subtitle_preference().await, None);
}

#[tokio::test]
async fn a_pick_in_no_version_is_no_such_version() {
    let alien = Alien::new().await;

    let picked = (alien.plans)
        .keep_pick(alien.user(), alien.version + 1, Pick::Subtitle(None))
        .await;

    assert!(matches!(picked, Err(PickError::NoSuchVersion)));
}

#[tokio::test]
async fn a_subtitle_offset_at_a_place_holding_audio_is_track_not_found() {
    let alien = Alien::new().await;

    let kept = (alien.plans)
        .keep_offset(alien.user(), alien.version, AUDIO, 500)
        .await;

    assert!(matches!(kept, Err(PickError::TrackNotFound)));
}
