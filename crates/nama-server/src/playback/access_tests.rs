use std::{
    collections::BTreeSet,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use axum::http::StatusCode;

use super::{Asked, Asker, Lease, PlaybackAccess};
use crate::{
    content_ratings::MaximumAge,
    db::{Database, now},
    events::EventBus,
    library::{
        Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
    playback::{Offers, RunningPlans, running::testing::direct_play_of},
    plugins::ExtensionPoints,
    server_settings::{PlaybackLimit, ServerUrl},
    signed_urls::{PlaybackGrant, SignedGrant, Viewer},
    users::{
        LibraryAccess, Permissions, User, Users,
        testing::{act_as, cap_remote_playback, insert_device, insert_user, insert_user_holding},
    },
};

const MADE_UP: i64 = 999_999;
const HOME: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
const AWAY: IpAddr = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));

/// What a request may have, as a row expects it: the Viewer a Version's file is leased to, `None`
/// for a lease with no file, or the status refusing it.
type Answer = Result<Option<Viewer>, StatusCode>;

fn answer(judged: Result<Lease, StatusCode>) -> Answer {
    judged.map(|lease| match lease {
        Lease::File(file) => Some(file.viewer),
        Lease::Media | Lease::Plan(_) => None,
    })
}

/// The Server's Users, Devices and plans every row is judged against, by who each is.
struct World {
    access: PlaybackAccess,
    running: RunningPlans,
    version: i64,
    /// Sees everything; plays Remuxes on her TV and laptop and Direct Plays on her TV.
    ada_tv: Viewer,
    ada_laptop: Viewer,
    /// Her TV's Remux, and her laptop's.
    tv_remux: u64,
    laptop_remux: u64,
    /// A Remux on her TV that stopped.
    stopped_remux: u64,
    /// A Device of Ada's that switched to Bob, one that switched to Bob and back, and one acting
    /// as no User.
    switched: Viewer,
    switched_back: Viewer,
    halfway: Viewer,
    /// Sees no Library.
    bob: Viewer,
    /// Has a Maximum Age the Version's R rating is past.
    cleo: Viewer,
    /// Each has a cap for remote playback: Dan plays nothing, Eve Direct Plays the Version as
    /// planned under it, and Fay as planned on the local network.
    dan: Viewer,
    eve: Viewer,
    fay: Viewer,
}

impl World {
    #[expect(
        clippy::too_many_lines,
        reason = "it sets out every User, Device and plan the rows are judged against"
    )]
    async fn new(db: &Arc<Database>, folder: &std::path::Path) -> Self {
        let rated_r = Metadata {
            content_rating: Some("R".to_owned()),
            ..tmdb_movie(348, "Alien")
        };
        let version = MatchedMovie::new(db, folder, rated_r).await.version;
        let users = Users::new(Arc::clone(db), ExtensionPoints::default());
        let admin = User {
            id: insert_user_holding(db, "Admin", Permissions::ADMIN).await,
            name: "Admin".to_owned(),
            permissions: Permissions::ADMIN,
            created_at: 0,
        };
        let sign_in = async |name| {
            let user = insert_user(db, name).await;
            let device = insert_device(db, user).await;
            Viewer { user, device }
        };
        let (ada_tv, bob, cleo) = (
            sign_in("Ada").await,
            sign_in("Bob").await,
            sign_in("Cleo").await,
        );
        let (dan, eve, fay) = (
            sign_in("Dan").await,
            sign_in("Eve").await,
            sign_in("Fay").await,
        );
        let ada_device = async || Viewer {
            device: insert_device(db, ada_tv.user).await,
            ..ada_tv
        };
        let (ada_laptop, switched, switched_back, halfway) = (
            ada_device().await,
            ada_device().await,
            ada_device().await,
            ada_device().await,
        );
        act_as(db, switched.device, Some(bob.user)).await;
        act_as(db, switched_back.device, Some(bob.user)).await;
        act_as(db, switched_back.device, Some(ada_tv.user)).await;
        act_as(db, halfway.device, None).await;
        let hidden = LibraryAccess::Only(BTreeSet::new());
        users
            .set_library_access(&admin, bob.user, hidden)
            .await
            .unwrap();
        let thirteen = MaximumAge {
            age: Some(13),
            allow_unrated: true,
        };
        users
            .set_maximum_age(&admin, cleo.user, thirteen)
            .await
            .unwrap();
        for capped in [dan, eve, fay] {
            cap_remote_playback(db, capped.user, PlaybackLimit::P720).await;
        }

        let events = EventBus::new(
            Arc::clone(db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let running = RunningPlans::for_tests(db, events, folder);
        let tv_remux = running.start_faked_remux(ada_tv, version).await;
        let laptop_remux = running.start_faked_remux(ada_laptop, version).await;
        let stopped_remux = running.start_faked_remux(ada_tv, version).await;
        running.stop_for(stopped_remux, ada_tv).await;
        let under_cap = crate::playback::Plan {
            offers: Offers {
                cap: Some(PlaybackLimit::P720),
                ..Offers::default()
            },
            ..direct_play_of(version)
        };
        for (viewer, plan) in [
            (ada_tv, direct_play_of(version)),
            (eve, under_cap),
            (fay, direct_play_of(version)),
        ] {
            running.start(viewer, plan, 0).await.unwrap();
        }
        Self {
            access: PlaybackAccess::new(running.clone(), Arc::clone(db)),
            running,
            version,
            ada_tv,
            ada_laptop,
            tv_remux,
            laptop_remux,
            stopped_remux,
            switched,
            switched_back,
            halfway,
            bob,
            cleo,
            dan,
            eve,
            fay,
        }
    }

    fn session(viewer: Viewer) -> Asker {
        Asker {
            session: Some(viewer),
            grant: None,
        }
    }

    /// A grant for `viewer`'s plan `plan` of the Version, signed to expire `expires_in` seconds
    /// from now.
    fn grant(&self, viewer: Viewer, plan: u64, expires_in: i64) -> Asker {
        let grant = PlaybackGrant {
            viewer,
            version: self.version,
            plan,
        };
        Asker {
            session: None,
            grant: Some(SignedGrant {
                grant,
                expires: now() + expires_in,
            }),
        }
    }
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "one table, a row per way a request may or may not get media"
)]
async fn only_a_viewer_who_may_have_media_gets_it_on_the_terms_its_plans_set() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let w = World::new(&db, data.path()).await;
    let (file, media) = (Asked::File(w.version), Asked::Media(w.version));
    let session = World::session;
    let (fresh, expired) = (300, -1);
    let unauthorized = Err(StatusCode::UNAUTHORIZED);
    let not_found = Err(StatusCode::NOT_FOUND);
    let forbidden = Err(StatusCode::FORBIDDEN);

    let rows: Vec<(&str, Asker, Asked, IpAddr, Answer)> = vec![
        (
            "a session that sees the Version",
            session(w.ada_tv),
            file,
            HOME,
            Ok(Some(w.ada_tv)),
        ),
        (
            "its subtitles too",
            session(w.ada_tv),
            media,
            HOME,
            Ok(None),
        ),
        (
            "hidden by Library access",
            session(w.bob),
            file,
            HOME,
            not_found,
        ),
        (
            "its subtitles hidden by Library access",
            session(w.bob),
            media,
            HOME,
            not_found,
        ),
        (
            "hidden by Maximum Age",
            session(w.cleo),
            file,
            HOME,
            not_found,
        ),
        (
            "its subtitles hidden by Maximum Age",
            session(w.cleo),
            media,
            HOME,
            not_found,
        ),
        (
            "a fresh grant",
            w.grant(w.ada_tv, w.tv_remux, fresh),
            file,
            HOME,
            Ok(Some(w.ada_tv)),
        ),
        (
            "a grant past its expiry while its plan runs for its Device",
            w.grant(w.ada_tv, w.tv_remux, expired),
            file,
            HOME,
            Ok(Some(w.ada_tv)),
        ),
        (
            "a grant past its expiry once its plan stopped",
            w.grant(w.ada_tv, w.stopped_remux, expired),
            file,
            HOME,
            unauthorized,
        ),
        (
            "a grant past its expiry while its plan runs on another Device",
            w.grant(w.ada_tv, w.laptop_remux, expired),
            file,
            HOME,
            unauthorized,
        ),
        (
            "a grant without a plan past its expiry",
            w.grant(w.ada_tv, 0, expired),
            file,
            HOME,
            unauthorized,
        ),
        (
            "a grant whose Device switched to another User",
            w.grant(w.switched, 0, fresh),
            file,
            HOME,
            unauthorized,
        ),
        (
            "a grant whose Device switched back",
            w.grant(w.switched_back, 0, fresh),
            file,
            HOME,
            Ok(Some(w.switched_back)),
        ),
        (
            "a grant whose Device acts as no User",
            w.grant(w.halfway, 0, fresh),
            file,
            HOME,
            unauthorized,
        ),
        (
            "a session and a grant, which acts as the session",
            Asker {
                session: Some(w.ada_laptop),
                ..w.grant(w.ada_tv, w.tv_remux, fresh)
            },
            file,
            HOME,
            Ok(Some(w.ada_laptop)),
        ),
        (
            "a capped Viewer at home",
            session(w.dan),
            file,
            HOME,
            Ok(Some(w.dan)),
        ),
        (
            "a capped Viewer away with no plan",
            session(w.dan),
            file,
            AWAY,
            forbidden,
        ),
        (
            "a capped Viewer away, for subtitles",
            session(w.dan),
            media,
            AWAY,
            Ok(None),
        ),
        (
            "a capped Viewer away with a Direct Play planned under the cap",
            session(w.eve),
            file,
            AWAY,
            Ok(Some(w.eve)),
        ),
        (
            "a capped Viewer away with a Direct Play planned at home",
            session(w.fay),
            file,
            AWAY,
            forbidden,
        ),
        (
            "a plan's files by its Viewer",
            session(w.ada_tv),
            Asked::Plan(w.tv_remux),
            HOME,
            Ok(None),
        ),
        (
            "a plan's files by another User on its Device",
            session(Viewer {
                user: w.bob.user,
                ..w.ada_tv
            }),
            Asked::Plan(w.tv_remux),
            HOME,
            not_found,
        ),
        (
            "a plan's files by its User on another Device",
            session(w.ada_laptop),
            Asked::Plan(w.tv_remux),
            HOME,
            not_found,
        ),
        (
            "a plan's files by a grant past its expiry while it runs",
            w.grant(w.ada_tv, w.tv_remux, expired),
            Asked::Plan(w.tv_remux),
            HOME,
            Ok(None),
        ),
        (
            "a download with a Direct Play",
            session(w.ada_tv),
            Asked::Download(w.version),
            HOME,
            Ok(Some(w.ada_tv)),
        ),
        (
            "a download without one",
            session(w.ada_laptop),
            Asked::Download(w.version),
            HOME,
            Err(StatusCode::CONFLICT),
        ),
        (
            "a made-up Version",
            session(w.ada_tv),
            Asked::File(MADE_UP),
            HOME,
            not_found,
        ),
        (
            "its subtitles",
            session(w.ada_tv),
            Asked::Media(MADE_UP),
            HOME,
            not_found,
        ),
        (
            "its download",
            session(w.ada_tv),
            Asked::Download(MADE_UP),
            HOME,
            not_found,
        ),
        (
            "a made-up plan",
            session(w.ada_tv),
            Asked::Plan(999),
            HOME,
            not_found,
        ),
        (
            "no Viewer, for a file",
            Asker::default(),
            file,
            HOME,
            unauthorized,
        ),
        (
            "no Viewer, for subtitles",
            Asker::default(),
            media,
            HOME,
            unauthorized,
        ),
        (
            "no Viewer, for a plan",
            Asker::default(),
            Asked::Plan(w.tv_remux),
            HOME,
            unauthorized,
        ),
        (
            "no Viewer, for a download",
            Asker::default(),
            Asked::Download(w.version),
            HOME,
            unauthorized,
        ),
    ];

    for (row, asker, asked, from, expected) in rows {
        let judged = answer(w.access.judge(asker, asked, Some(from)).await);

        assert_eq!(judged, expected, "{row}");
    }
    w.running.shutdown().await;
}

#[test]
fn a_path_under_a_version_or_a_plan_or_a_versions_download_is_judged_and_any_other_is_not() {
    let judged = |path| Asked::of(path).map(|asked| format!("{asked:?}"));

    assert_eq!(judged("/media/versions/3").as_deref(), Some("Ok(File(3))"));
    assert_eq!(
        judged("/media/versions/3/fonts/1").as_deref(),
        Some("Ok(Media(3))")
    );
    assert_eq!(
        judged("/media/plans/5/index.m3u8").as_deref(),
        Some("Ok(Plan(5))")
    );
    assert_eq!(
        judged("/media/downloads/3").as_deref(),
        Some("Ok(Download(3))")
    );
    assert_eq!(
        judged("/media/versions/x/fonts/1").as_deref(),
        Some("Err(400)")
    );
    assert_eq!(judged("/media/downloads/transcodes/1"), None);
    assert_eq!(
        judged("/media/downloads/3/transcodes/PLAYBACK_LIMIT_720P"),
        None
    );
    assert_eq!(judged("/media/images/poster.jpg"), None);
}
