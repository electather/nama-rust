use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::Arc,
};

use futures_util::{Stream, StreamExt as _};
use nama_proto::nama::v1::{
    Playback, PlaybackMethod, PlaybackReasonKind, SubscribeResponse, event::Event as E,
    subscribe_response,
};
use tempfile::TempDir;
use tokio::task::JoinHandle;

use super::{Asked, AskedSubtitle, Pick, PickError, Plans};
use crate::{
    db::Database,
    events::{EventBus, Filter},
    ffmpeg::{Abilities, CanDo, Ffmpeg},
    images::ImageStore,
    library::{
        Contents, Decided,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{
        Description, DeviceDecoding, Method, PickedLimit, Plan, Planned, Preferred, PreviewFrames,
        Probing, RunningPlans, StartError, StartedPlan, SubtitlePreference, TrackPreferences,
        probing::testing::{FakeReader, failed},
        running::testing::{Converted, FakeRuns},
        tests::{MKV, audio, firefox, track, video},
        tracks::{ListedKeyframes, Probe, Track, TrackKind},
    },
    plugins::{ExtensionPoints, Registry},
    server_settings::{HardwareChoice, PlaybackChange, ServerUrl, Settings},
    users::{Caller, Device, Permissions, User, Users},
};

/// The place of the audio Track in [`movie`].
const AUDIO: usize = 1;
/// The place of the subtitle Track in [`movie`].
const SUBTITLE: usize = 2;

/// A Matroska file a minute long holding H.264 video, AAC audio and an English `SubRip`
/// subtitle, which Firefox plays only Remuxed or Transcoded.
fn movie() -> Probe {
    let subtitle = TrackKind::Subtitle {
        image: false,
        header: false,
        sidecar: None,
    };
    let english = Track {
        language: Some("eng".to_owned()),
        ..track("subrip", false, subtitle)
    };
    Probe {
        container: MKV.to_owned(),
        duration_ms: Some(60_000),
        bitrate: Some(5_000_000),
        tracks: vec![video("h264"), audio("aac", true), english],
        font_attachments: vec![],
        chapters: vec![],
    }
}

/// [`movie`]'s keyframes, one every 2 seconds.
fn keyframes() -> ListedKeyframes {
    ListedKeyframes {
        file_start_ms: 0,
        keyframes_ms: (0..30).map(|n| n * 2000).collect(),
    }
}

/// What a Server whose ffmpeg Remuxes and Transcodes in software can do.
fn remuxing_and_transcoding() -> CanDo {
    CanDo {
        runs: true,
        zscale: true,
        tonemapx: false,
        subtitle_layer: false,
        libx264: true,
        hardware: vec![],
        gpus: vec![],
    }
}

/// Plans on a database in a temp folder, with Ripley's Device and a Matched Movie whose Version's
/// Tracks aren't probed yet, read as [`movie`] by an in-memory reader that lists [`keyframes`],
/// and run by in-memory runs. The Version is stored without a scan, so no background pass probes
/// it.
struct Alien {
    plans: Plans,
    db: Arc<Database>,
    reader: Arc<FakeReader>,
    runs: FakeRuns,
    settings: Settings,
    events: EventBus,
    ripley: Caller,
    item: i64,
    version: i64,
    data: TempDir,
    _media: TempDir,
}

impl Alien {
    async fn new() -> Self {
        let (data, media) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let alien = MatchedMovie::new(&db, media.path(), tmdb_movie(348, "Alien")).await;
        let reader = FakeReader::answering(|_| Ok(movie()));
        reader.list(|_| Ok(keyframes()));
        let probing = Probing::reading_with(Arc::clone(&db), Arc::clone(&reader));
        let settings = Settings::new(Arc::clone(&db), data.path().join("transcodes"));
        let events = EventBus::new(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let runs = FakeRuns::default();
        let running = RunningPlans::on_fake_runs(
            &runs,
            settings.playback(),
            probing.clone(),
            &db,
            events.clone(),
            data.path(),
        );
        let preview_frames = PreviewFrames::new(
            Arc::clone(&db),
            ImageStore::new(data.path(), Arc::clone(&db)),
            Arc::new(Ffmpeg::default()),
            running.clone(),
            Arc::default(),
        );
        let plans = Plans::new(
            probing,
            Abilities::fixed(remuxing_and_transcoding()),
            running,
            settings.playback(),
            preview_frames,
            TrackPreferences::new(Arc::clone(&db)),
            Arc::clone(&db),
        );
        let ripley = ripley_on_the_nostromo(&db).await;
        Self {
            plans,
            db,
            reader,
            runs,
            settings,
            events,
            ripley,
            item: alien.item,
            version: alien.version,
            data,
            _media: media,
        }
    }

    fn user(&self) -> i64 {
        self.ripley.user.id
    }

    /// What a request for the Movie from a Device that decodes `decoding` asks, after `failed`
    /// failed to play it there, leaving the rest to the Server and turning subtitles off unless
    /// it asks `subtitle`.
    fn asked(&self, decoding: DeviceDecoding, failed: &[Method], subtitle: AskedSubtitle) -> Asked {
        Asked {
            version: self.version,
            decoding,
            failed: failed.iter().copied().collect(),
            failed_recipes: BTreeSet::new(),
            audio: None,
            subtitle,
            limit: PickedLimit::Unpicked,
            start_ms: 0,
            client: IpAddr::from(Ipv4Addr::LOCALHOST),
        }
    }

    /// The plan of the Movie on a Device that plays nothing, asking `subtitle` of subtitles and
    /// leaving the rest to the Server.
    async fn plan(&self, subtitle: AskedSubtitle) -> Plan {
        let asked = self.asked(DeviceDecoding::default(), &[], subtitle);
        match self.plans.plan(&self.ripley, asked).await.unwrap() {
            Some(Planned::Started(StartedPlan { plan, .. }) | Planned::Unplayable { plan, .. }) => {
                plan
            }
            None => panic!("no such Version"),
        }
    }

    /// Plans the Movie for Firefox on `caller`'s Device, after `failed` failed to play it there,
    /// in the background.
    fn play_as(
        &self,
        caller: &Caller,
        failed: &[Method],
    ) -> JoinHandle<Result<Option<Planned>, StartError>> {
        let (plans, caller) = (self.plans.clone(), caller.clone());
        let asked = self.asked(firefox(), failed, AskedSubtitle::None);
        tokio::spawn(async move { plans.plan(&caller, asked).await })
    }

    /// Plans the Movie for Firefox on the Nostromo, in the background.
    fn play(&self) -> JoinHandle<Result<Option<Planned>, StartError>> {
        self.play_as(&self.ripley, &[])
    }

    /// Probes the Movie and lists its keyframes, as a play before would have.
    async fn probe_and_list(&self) {
        self.plans.probing.current(self.version).await.unwrap();
        self.plans.probing.keyframes(self.version).await.unwrap();
    }

    async fn allow_one_transcode(&self) {
        let change = PlaybackChange {
            segment_folder: "",
            max_transcodes: Some(1),
            keep_behind_minutes: None,
            hardware: HardwareChoice::Automatic,
            remote_limit: None,
            subtitle_cache_mib: None,
        };
        self.settings.change_playback(change, &[]).await.unwrap();
    }

    /// What's in the segment folder.
    fn segment_folder(&self) -> Vec<PathBuf> {
        let folder = self.data.path().join("transcodes");
        std::fs::read_dir(folder).map_or_else(
            |_| vec![],
            |entries| entries.map(|entry| entry.unwrap().path()).collect(),
        )
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

/// What `planning` answers once `runs` started `count` runs, the last of which writes nothing: on
/// paused time from then, so its wait for a first file ends at once, as it would for a slow but
/// healthy ffmpeg. Time is paused only then, as the database answers from a thread of its own,
/// which paused time would skip timers past.
async fn once_started<T>(planning: JoinHandle<T>, runs: &FakeRuns, count: usize) -> T {
    runs.until_started(count).await;
    tokio::time::pause();
    let planned = planning.await.unwrap();
    tokio::time::resume();
    planned
}

/// The plan `planned` started.
fn started(planned: Result<Option<Planned>, StartError>) -> Plan {
    match planned {
        Ok(Some(Planned::Started(StartedPlan { plan, .. }))) => plan,
        other => panic!("not started: {other:?}"),
    }
}

/// The plan `planned` found unplayable.
fn unplayable(planned: Result<Option<Planned>, StartError>) -> Plan {
    match planned {
        Ok(Some(Planned::Unplayable { plan, .. })) => plan,
        other => panic!("not unplayable: {other:?}"),
    }
}

/// The methods `plan`'s reasons say failed on the Server, in order.
fn failed_on_server(plan: &Plan) -> Vec<&str> {
    (plan.reasons.iter())
        .filter(|reason| {
            reason.kind == PlaybackReasonKind::PLAYBACK_REASON_KIND_METHOD_FAILED_ON_SERVER
        })
        .flat_map(|reason| &reason.params)
        .filter(|(key, _)| *key == "method")
        .map(|(_, method)| method.as_str())
        .collect()
}

/// The playback `subscription` next hears started.
async fn next_started(
    subscription: &mut (impl Stream<Item = SubscribeResponse> + Unpin),
) -> Playback {
    let next = subscription.next().await.unwrap().message;
    let Some(subscribe_response::Message::Event(event)) = next else {
        panic!("not an event: {next:?}");
    };
    let Some(E::PlaybackStarted(started)) = event.event else {
        panic!("not a PlaybackStarted: {event:?}");
    };
    started.playback.unwrap()
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

/// `caller` on another Device, the Narcissus.
fn on_the_narcissus(caller: &Caller) -> Caller {
    Caller {
        device: Device {
            id: 2,
            name: "Narcissus".to_owned(),
            ..caller.device.clone()
        },
        ..caller.clone()
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

#[tokio::test]
async fn a_remux_whose_run_fails_at_once_transcodes_saying_why_and_only_the_transcode_is_told() {
    let alien = Alien::new().await;
    alien.runs.fail_at_once(Converted::Remux);
    let mut subscription = Box::pin(alien.events.subscribe_as_admin(Filter::default()));

    let plan = started(once_started(alien.play(), &alien.runs, 2).await);

    assert_eq!(plan.method(), Some(Method::Transcode));
    assert_eq!(failed_on_server(&plan), ["PLAYBACK_METHOD_REMUX"]);
    let told = next_started(&mut subscription).await;
    assert_eq!(told.method, PlaybackMethod::PLAYBACK_METHOD_TRANSCODE);
    assert_eq!(
        alien.runs.converted(),
        [Converted::Remux, Converted::TranscodeInSoftware]
    );
}

#[tokio::test]
async fn a_version_whose_remux_and_transcode_fail_at_once_is_unplayable_naming_both_leaving_no_folder()
 {
    let alien = Alien::new().await;
    alien.runs.fail_at_once(Converted::Remux);
    alien.runs.fail_at_once(Converted::TranscodeInSoftware);

    let plan = unplayable(alien.play().await.unwrap());

    assert_eq!(
        failed_on_server(&plan),
        ["PLAYBACK_METHOD_REMUX", "PLAYBACK_METHOD_TRANSCODE"]
    );
    assert_eq!(alien.segment_folder(), Vec::<PathBuf>::new());
}

#[tokio::test]
async fn transcodes_that_fail_at_once_dont_count_toward_the_transcode_limit() {
    let alien = Alien::new().await;
    alien.allow_one_transcode().await;
    alien.runs.fail_at_once(Converted::TranscodeInSoftware);

    let first = alien
        .play_as(&alien.ripley, &[Method::Remux])
        .await
        .unwrap();
    let second = alien
        .play_as(&alien.ripley, &[Method::Remux])
        .await
        .unwrap();

    for plan in [unplayable(first), unplayable(second)] {
        assert_eq!(failed_on_server(&plan), ["PLAYBACK_METHOD_TRANSCODE"]);
    }
}

#[tokio::test]
async fn a_remux_whose_keyframes_cant_be_listed_transcodes_saying_why() {
    let alien = Alien::new().await;
    alien.reader.list(|_| Err(failed()));

    let plan = started(once_started(alien.play(), &alien.runs, 1).await);

    assert_eq!(plan.method(), Some(Method::Transcode));
    let unlisted = PlaybackReasonKind::PLAYBACK_REASON_KIND_KEYFRAMES_UNLISTED;
    assert!(plan.reasons.iter().any(|reason| reason.kind == unlisted));
    assert_eq!(alien.runs.converted(), [Converted::TranscodeInSoftware]);
}

#[tokio::test]
async fn a_remux_whose_keyframes_cant_be_listed_while_the_transcode_limit_is_full_is_busy() {
    let alien = Alien::new().await;
    alien.allow_one_transcode().await;
    alien.reader.list(|_| Err(failed()));
    let other_device = on_the_narcissus(&alien.ripley);
    let transcoding = alien.play_as(&other_device, &[Method::Remux]);
    started(once_started(transcoding, &alien.runs, 1).await);

    let planned = alien.play().await.unwrap();

    assert!(matches!(planned, Err(StartError::Busy(1))), "{planned:?}");
}

#[tokio::test]
async fn a_transcode_started_while_a_remuxs_keyframes_are_listed_counts_before_it_transcodes() {
    let alien = Alien::new().await;
    alien.allow_one_transcode().await;
    alien.plans.probing.current(alien.version).await.unwrap();
    alien.reader.list(|_| Err(failed()));
    alien.reader.hold();
    let remuxing = alien.play();
    alien.reader.until_listed(1).await;

    let other_device = on_the_narcissus(&alien.ripley);
    let transcoding = alien.play_as(&other_device, &[Method::Remux]);
    started(once_started(transcoding, &alien.runs, 1).await);
    alien.reader.release();

    let planned = remuxing.await.unwrap();
    assert!(matches!(planned, Err(StartError::Busy(1))), "{planned:?}");
}

#[tokio::test]
async fn a_caller_that_stops_waiting_mid_start_leaves_no_run_going_without_a_plan() {
    let alien = Alien::new().await;
    alien.probe_and_list().await;
    let planning = alien.play();
    alien.runs.until_started(1).await;

    planning.abort();
    assert!(planning.await.unwrap_err().is_cancelled());
    // Past the wait for its first file, which the start carries on with or was dropped from.
    tokio::time::pause();
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    tokio::time::resume();

    let kept = alien.plans.running.playing(&[alien.version]) == 1;
    let dropped = alien.runs.dropped(0) && alien.segment_folder().is_empty();
    assert!(kept != dropped, "kept: {kept}, dropped: {dropped}");
}
