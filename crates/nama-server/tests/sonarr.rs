//! The Sonarr Installed Plugin, bundled in the Server, against a fake Sonarr: its settings check
//! on the Plugins page, Season Requests handed to it once Approved, and their download progress.

#[expect(dead_code, reason = "Sonarr's tests remove no quality profile")]
#[path = "support/fake_servarr.rs"]
mod fake_servarr;
#[expect(dead_code, reason = "this binary deletes no media")]
#[path = "support/fake_sonarr.rs"]
mod fake_sonarr;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/servarr.rs"]
mod servarr;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::ErrorCode;
use fake_sonarr::{API_KEY as SONARR_KEY, FakeSonarr};
use nama_plugin_api::fulfiller::Shows;
use nama_proto::nama::v1::{
    CancelFollowedShowRequest, Download, DownloadStatus, EventType, RefusalReason, Request,
    RequestState, event,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use servarr::{ServarrFixture, choices};
use server::{
    SignedIn, Subscription, TestServer,
    fulfiller::quality,
    requests::Requester,
    subscribe,
    tmdb::{DARK, DARK_TVDB, THE_OFFICE},
};

/// Quality profiles the fake Sonarr offers, by id.
const HD_1080P: &str = "4";
const ULTRA_HD: &str = "5";

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Sonarr that has no
/// Shows yet.
struct Fixture {
    server: TestServer,
    /// The Server's Plugins, to ask Sonarr's Fulfiller directly.
    plugins: Registry,
    sonarr: FakeSonarr,
}

impl ServarrFixture for Fixture {
    const PLUGIN: &'static str = "sonarr";
    type Kind = Shows;

    fn server(&self) -> &TestServer {
        &self.server
    }

    fn registry(&self) -> &Registry {
        &self.plugins
    }

    fn servarr_url(&self) -> String {
        self.sonarr.url()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let plugins = Registry::new();
        let server = TestServer::builder()
            .plugins(plugins.clone())
            .tmdb()
            .start()
            .await;
        Self {
            server,
            plugins,
            sonarr: FakeSonarr::start().await,
        }
    }

    /// A Fixture where an Admin set Sonarr up with the profile HD-1080p and the root folder `/tv`,
    /// and enabled it.
    async fn with_sonarr_enabled() -> Self {
        let fixture = Self::start().await;
        fixture.sonarr.enable_on(&fixture.server).await;
        fixture
    }

    /// The Seasons `seasons` of the Show `show`, asked for by the Admin and so Approved at once, as
    /// their hand-offs left them.
    async fn ask_approved(&self, show: i64, seasons: &[u32]) -> Vec<Request> {
        self.server
            .admin_requests()
            .ask_seasons(show, seasons)
            .await
    }

    /// The whole Show `show`, asked for by the Admin and so Approved at once. Returns the Followed
    /// Show's id and its Season Requests as their hand-offs left them.
    async fn follow_approved(&self, show: i64) -> (i64, Vec<Request>) {
        let (followed, asked) = self.server.admin_requests().follow(show).await;
        (followed.id, asked)
    }

    /// Cancels the Followed Show `id` as the Admin.
    async fn cancel_followed(&self, id: i64) {
        self.server
            .admin_requests()
            .cancel_followed_show(CancelFollowedShowRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The Season `season` of the Show `show`, asked for by the new User `member` and so Pending.
    async fn ask_pending(&self, member: &SignedIn, show: i64, season: u32) -> Request {
        let asked = self
            .server
            .requests(member)
            .ask_seasons(show, &[season])
            .await
            .remove(0);
        assert_eq!(asked.state, RequestState::Pending);
        asked
    }

    /// Forgets every Season Request's Show TVDB id, as for a Show the metadata source has none
    /// for.
    async fn forget_show_tvdb_ids(&self) {
        self.server
            .db()
            .write(|tx| {
                Box::pin(async move {
                    sqlx::query("UPDATE requests SET show_tvdb_id = NULL")
                        .execute(&mut **tx)
                        .await
                })
            })
            .await
            .unwrap();
    }

    /// Subscribes as the Admin to the progress of every Request.
    async fn subscribe_to_progress(&self) -> Subscription {
        subscribe(
            &self.server.client_config(),
            &[EventType::EVENT_TYPE_REQUEST_PROGRESS],
        )
        .await
    }
}

fn states(requests: &[Request]) -> Vec<Option<RequestState>> {
    requests
        .iter()
        .map(|request| request.state.as_known())
        .collect()
}

#[tokio::test]
async fn testing_sonarrs_settings_offers_its_quality_profiles_and_root_folders() {
    let fixture = Fixture::start().await;

    let tested = fixture.test_settings(SONARR_KEY).await.unwrap();

    assert_eq!(
        choices(&tested, "quality_profile"),
        [("1", "Any"), ("4", "HD-1080p"), ("5", "Ultra-HD")]
    );
    assert_eq!(
        choices(&tested, "root_folder"),
        [("/tv", "/tv"), ("/anime", "/anime")]
    );
}

#[tokio::test]
async fn testing_sonarrs_settings_with_a_wrong_key_or_sonarr_down_fails_in_plain_words() {
    let mut fixture = Fixture::start().await;

    let wrong_key = fixture.test_settings("not-the-key").await;
    fixture.sonarr.go_down();
    let down = fixture.test_settings(SONARR_KEY).await;

    for (failed, message) in [
        (wrong_key, "Sonarr refused the API key"),
        (down, "Sonarr didn't answer"),
    ] {
        let shown = failed.as_ref().err().and_then(|err| err.message.clone());
        assert_refused(
            failed,
            ErrorCode::FailedPrecondition,
            RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
        );
        assert_eq!(shown.as_deref(), Some(message));
    }
}

#[tokio::test]
async fn asking_for_seasons_of_a_show_sonarr_lacks_adds_it_with_only_those_seasons_monitored_and_searches_each()
 {
    let fixture = Fixture::with_sonarr_enabled().await;

    let asked = fixture.ask_approved(DARK, &[1, 3]).await;
    fixture.sonarr.finish_refreshes().await;

    assert_eq!(states(&asked), [Some(RequestState::Processing); 2]);
    let shows = fixture.sonarr.shows();
    let [dark] = shows.as_slice() else {
        panic!("Sonarr has {} Shows, not just Dark", shows.len());
    };
    assert_eq!(dark["tvdbId"], DARK_TVDB);
    assert_eq!(dark["qualityProfileId"], 4);
    assert_eq!(dark["rootFolderPath"], "/tv");
    assert_eq!(dark["monitorNewItems"], "none");
    assert_eq!(dark["addOptions"]["monitor"], "skip");
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 3]);
    assert_eq!(fixture.sonarr.searches(), [(DARK_TVDB, 1), (DARK_TVDB, 3)]);
    assert_eq!(fixture.sonarr.monitored_episodes(), [301, 302]);
}

#[tokio::test]
async fn a_later_season_request_adds_its_season_to_the_monitored_ones_without_changing_the_others()
{
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.ask_approved(DARK, &[1, 3]).await;

    let asked = fixture.ask_approved(DARK, &[2]).await;
    fixture.sonarr.finish_refreshes().await;

    assert_eq!(states(&asked), [Some(RequestState::Processing)]);
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 2, 3]);
    assert_eq!(fixture.sonarr.searches().last(), Some(&(DARK_TVDB, 2)));
}

#[tokio::test]
async fn a_show_another_hand_off_added_meanwhile_gets_the_season_monitored_rather_than_failing() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.answer_next_find_stale();

    let asked = fixture.ask_approved(DARK, &[3]).await;
    fixture.sonarr.finish_refreshes().await;

    assert_eq!(states(&asked), [Some(RequestState::Processing)]);
    assert_eq!(fixture.sonarr.shows().len(), 1);
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 3]);
}

#[tokio::test]
async fn a_season_handed_off_while_sonarr_is_still_adding_its_show_fails_in_plain_words() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.sonarr.hold_refreshes();

    let asked = fixture.ask_approved(DARK, &[1, 3]).await;

    assert_eq!(
        states(&asked),
        [Some(RequestState::Processing), Some(RequestState::Failed)]
    );
    assert_eq!(
        asked[1].failure_reason.as_deref(),
        Some(
            "Sonarr was still adding the Show, so it would have forgotten this Season. \
             Retry in a minute."
        )
    );
}

#[tokio::test]
async fn with_sonarr_down_a_request_fails_in_plain_words_and_retrying_once_it_is_back_moves_it_to_processing()
 {
    let mut fixture = Fixture::with_sonarr_enabled().await;
    fixture.sonarr.go_down();

    let failed = fixture.ask_approved(DARK, &[1]).await.remove(0);
    fixture.sonarr.come_back().await;
    let retried = fixture.server.retry(failed.id).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Sonarr didn't answer")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1]);
}

#[tokio::test]
async fn a_season_of_a_show_with_no_tvdb_id_fails_in_plain_words() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let ada = fixture.server.user_with_default_permissions("Ada").await;
    let pending = fixture.ask_pending(&ada, DARK, 1).await;
    fixture.forget_show_tvdb_ids().await;

    let failed = fixture.server.approve(pending.id).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Sonarr needs the Show's TVDB id, and none is known for it")
    );
    assert_eq!(fixture.sonarr.shows(), Vec::<serde_json::Value>::new());
}

#[tokio::test]
async fn a_show_sonarr_cant_find_fails_in_plain_words() {
    let fixture = Fixture::with_sonarr_enabled().await;

    let failed = fixture.ask_approved(THE_OFFICE, &[1]).await.remove(0);

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Sonarr couldn't find the Show by its TVDB id, 73244")
    );
}

/// The Request and its downloads in the next progress event on `subscription`.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_progress(subscription: &mut Subscription) -> (i64, Vec<Download>) {
    let event = subscription.next().await;
    let event::Event::RequestProgress(progress) = event else {
        panic!("not progress: {event:?}");
    };
    let request = progress
        .request
        .into_option()
        .expect("progress of no Request");
    (request.id, request.downloads)
}

#[tokio::test]
async fn processing_season_requests_show_sonarrs_queue_for_their_season_only_asked_once_per_poll() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.sonarr.set_queue("queue.json");
    let mut progress = fixture.subscribe_to_progress().await;

    let first_season = fixture.ask_approved(DARK, &[1]).await.remove(0);
    let first = next_progress(&mut progress).await;
    fixture.sonarr.finish_refreshes().await;
    fixture.sonarr.set_queue("queue-later.json");
    // Becoming Processing wakes the poll at once, rather than in a minute.
    let third_season = fixture.ask_approved(DARK, &[3]).await.remove(0);
    let mut later = [
        next_progress(&mut progress).await,
        next_progress(&mut progress).await,
    ];
    later.sort_by_key(|(id, _)| *id);

    let pack_at = |percent, eta_seconds| Download {
        title: "Dark.S01.1080p.NF.WEB-DL.x264".to_owned(),
        percent,
        size_bytes: Some(20_000_000_000),
        eta_seconds: Some(eta_seconds),
        status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
        ..Default::default()
    };
    let third_season_episodes = vec![
        Download {
            title: "Dark.S03E01.1080p.NF.WEB-DL.x264".to_owned(),
            percent: 75.0,
            size_bytes: Some(2_000_000_000),
            eta_seconds: Some(600),
            status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
            ..Default::default()
        },
        Download {
            title: "Dark.S03E02.1080p.NF.WEB-DL.x264".to_owned(),
            percent: 100.0,
            size_bytes: Some(2_000_000_000),
            status: DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
            attention_reason: Some(
                "Sonarr said: Not an upgrade for existing episode file(s)".to_owned(),
            ),
            ..Default::default()
        },
    ];
    assert_eq!(first, (first_season.id, vec![pack_at(25.0, 93_600)]));
    assert_eq!(
        later,
        [
            (first_season.id, vec![pack_at(60.0, 2_700)]),
            (third_season.id, third_season_episodes)
        ]
    );
    assert_eq!(fixture.sonarr.queue_calls(), 2);
}

#[tokio::test]
async fn with_sonarr_refusing_the_key_or_down_progress_fails_in_plain_words() {
    let mut fixture = Fixture::with_sonarr_enabled().await;
    fixture.sonarr.set_queue("queue.json");
    let first_season = [format!("{DARK_TVDB}/1")];

    let answered = fixture.progress(&first_season).await;
    fixture.sonarr.refuse_key();
    let refused = fixture.progress(&first_season).await;
    fixture.sonarr.go_down();
    let down = fixture.progress(&first_season).await;

    assert_eq!(answered, Ok(first_season.to_vec()));
    assert_eq!(refused, Err("Sonarr refused the API key".to_owned()));
    assert_eq!(down, Err("Sonarr didn't answer".to_owned()));
}

#[tokio::test]
async fn asking_for_a_whole_show_sonarr_lacks_adds_it_with_new_seasons_monitored() {
    let fixture = Fixture::with_sonarr_enabled().await;

    let (_, asked) = fixture.follow_approved(DARK).await;
    fixture.sonarr.finish_refreshes().await;

    assert_eq!(states(&asked), [Some(RequestState::Processing); 3]);
    let shows = fixture.sonarr.shows();
    let [dark] = shows.as_slice() else {
        panic!("Sonarr has {} Shows, not just Dark", shows.len());
    };
    assert_eq!(dark["monitorNewItems"], "all");
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 2, 3]);
}

#[tokio::test]
async fn asking_for_a_whole_show_sonarr_has_for_one_season_turns_on_its_new_season_monitoring() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.finish_refreshes().await;

    fixture.follow_approved(DARK).await;

    assert_eq!(fixture.sonarr.shows()[0]["monitorNewItems"], "all");
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 2, 3]);
}

#[tokio::test]
async fn cancelling_a_followed_show_turns_off_sonarrs_new_season_monitoring() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let (followed, _) = fixture.follow_approved(DARK).await;
    fixture.sonarr.finish_refreshes().await;

    fixture.cancel_followed(followed).await;

    assert_eq!(fixture.sonarr.shows()[0]["monitorNewItems"], "none");
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 2, 3]);
}

#[tokio::test]
async fn a_new_series_is_added_to_sonarr_at_its_requests_profile() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture
        .server
        .replace_qualities("sonarr", vec![quality(ULTRA_HD, "4K", true)])
        .await;

    fixture.ask_approved(DARK, &[1]).await;

    assert_eq!(fixture.sonarr.shows()[0]["qualityProfileId"], 5);
}

#[tokio::test]
async fn the_hand_off_reports_the_profile_sonarr_kept_for_a_series_it_already_had() {
    let fixture = Fixture::with_sonarr_enabled().await;
    // Added at the setting's profile, HD-1080p, before the Admin offered any Quality.
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.finish_refreshes().await;
    fixture
        .server
        .replace_qualities("sonarr", vec![quality(ULTRA_HD, "4K", true)])
        .await;

    let asked = fixture.ask_approved(DARK, &[2]).await.remove(0);

    assert_eq!(asked.state, RequestState::Processing);
    let used = &asked.quality_used;
    assert_eq!(
        (used.name.as_str(), used.profile_id.as_str()),
        ("HD-1080p", HD_1080P)
    );
}
