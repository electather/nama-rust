//! The Radarr Installed Plugin, bundled in the Server, against a fake Radarr: its settings check
//! on the Plugins page, Movie Requests handed to it once Approved, and their download progress.

#[path = "support/fake_radarr.rs"]
mod fake_radarr;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/servarr.rs"]
mod servarr;
#[expect(dead_code, reason = "the harness's `data_dir` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use fake_radarr::{API_KEY as RADARR_KEY, FakeRadarr};
use nama_plugin_api::{
    fulfiller::Movies,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveRequestRequest, CreateRequestRequest,
    CreateUserRequest, Download, DownloadStatus, EnablePluginRequest, EventServiceClient,
    EventType, Quality, RefusalReason, ReplaceQualitiesRequest, Request, RequestServiceClient,
    RequestState, RetryRequestRequest, SessionDelivery, SignInRequest, SignInServiceClient,
    SubscribeRequest, SubscribeResponse, UpdatePluginSettingsRequest, event, subscribe_response,
};
use nama_server::{config::Config, plugins::Registry};
use refused::assert_refused;
use serde_json::json;
use servarr::{ServarrFixture, choices, quality, text};
use server::{PASSWORD, TestServer};

/// Quality profiles the fake Radarr offers, by id.
const ANY: &str = "1";
const HD_1080P: &str = "4";
const ULTRA_HD: &str = "5";

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const HEAT: i64 = 949;
const DUNE: i64 = 438_631;

/// How long to wait for the Server to tell of progress.
const WAIT: Duration = Duration::from_secs(10);

type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Radarr that already
/// has Alien and can't find Dune.
struct Fixture {
    server: TestServer,
    /// The Server's Plugins, to ask Radarr's Fulfiller directly.
    plugins: Registry,
    radarr: FakeRadarr,
    _tmdb: FakeTmdb,
}

impl ServarrFixture for Fixture {
    const PLUGIN: &'static str = "radarr";
    type Kind = Movies;

    fn server(&self) -> &TestServer {
        &self.server
    }

    fn registry(&self) -> &Registry {
        &self.plugins
    }

    fn servarr_url(&self) -> String {
        self.radarr.url()
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    async fn start() -> Self {
        let tmdb = FakeTmdb::start().await;
        let plugins = Registry::new();
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(TMDB_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let server = TestServer::start(Config::default(), plugins.clone()).await;
        Self {
            server,
            plugins,
            radarr: FakeRadarr::start(&[ALIEN], &[DUNE]).await,
            _tmdb: tmdb,
        }
    }

    /// A Fixture where an Admin set Radarr up with the profile HD-1080p and the root folder
    /// `/movies`, and enabled it.
    async fn with_radarr_enabled() -> Self {
        Self::with_radarr_enabled_at(HD_1080P).await
    }

    /// [`Self::with_radarr_enabled`], with the Radarr quality profile `profile` as its setting.
    async fn with_radarr_enabled_at(profile: &str) -> Self {
        let fixture = Self::start().await;
        let mut settings = fixture.settings(RADARR_KEY);
        settings.extend([("quality_profile", profile), ("root_folder", "/movies")].map(text));
        fixture
            .plugins()
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "radarr".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
            .plugins()
            .enable_plugin(EnablePluginRequest {
                id: "radarr".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
    }

    /// The Movie `tmdb_id`, asked for by the new User `member` and so Pending, then Approved by
    /// the Admin, as the approval left it.
    async fn approve_movie(&self, member: &ClientConfig, tmdb_id: i64) -> Request {
        let asked = RequestServiceClient::new(HttpClient::plaintext(), member.clone())
            .create_request(CreateRequestRequest {
                movie_tmdb_id: tmdb_id,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .request
            .into_option()
            .unwrap();
        assert_eq!(asked.state, RequestState::Pending);
        let approved = self
            .requests()
            .approve_request(ApproveRequestRequest {
                id: asked.id,
                ..Default::default()
            })
            .await
            .unwrap();
        approved.into_owned().request.into_option().unwrap()
    }

    /// Replaces Radarr's Qualities with `qualities` as the Admin.
    async fn replace_qualities(&self, qualities: Vec<Quality>) {
        self.plugins()
            .replace_qualities(ReplaceQualitiesRequest {
                id: "radarr".to_owned(),
                qualities,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// Retries the Request `id` as the Admin, and returns it as the retry left it.
    async fn retry(&self, id: i64) -> Request {
        let retried = self
            .requests()
            .retry_request(RetryRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
        retried.into_owned().request.into_option().unwrap()
    }

    /// Subscribes as the Admin to the progress of every Request.
    async fn subscribe_to_progress(&self) -> Subscription {
        let only = SubscribeRequest {
            types: vec![EventType::EVENT_TYPE_REQUEST_PROGRESS.into()],
            ..Default::default()
        };
        EventServiceClient::new(HttpClient::plaintext(), self.server.client_config())
            .subscribe(only)
            .await
            .unwrap()
    }

    /// Creates a User with the Permissions a new User gets, signs them in, and returns client
    /// settings that call as them.
    async fn member(&self) -> ClientConfig {
        nama_proto::nama::v1::UserServiceClient::new(
            HttpClient::plaintext(),
            self.server.client_config(),
        )
        .create_user(CreateUserRequest {
            name: "Ada".to_owned(),
            password: PASSWORD.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
        let signed_in = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        )
        .sign_in(SignInRequest {
            name: "Ada".to_owned(),
            password: PASSWORD.to_owned(),
            device_name: "Phone".to_owned(),
            delivery: SessionDelivery::SESSION_DELIVERY_TOKEN.into(),
            ..Default::default()
        })
        .await
        .unwrap();
        self.server.client_config_as(&signed_in.into_owned().token)
    }
}

/// "HD" (HD-1080p, the default) and "4K" (Ultra-HD), as an Admin offers them.
fn hd_and_4k() -> Vec<Quality> {
    vec![
        quality(HD_1080P, "HD", true),
        quality(ULTRA_HD, "4K", false),
    ]
}

#[tokio::test]
async fn testing_radarrs_settings_offers_its_quality_profiles_and_root_folders() {
    let fixture = Fixture::start().await;

    let tested = fixture.test_settings(RADARR_KEY).await.unwrap();

    assert_eq!(
        choices(&tested, "quality_profile"),
        [("1", "Any"), ("4", "HD-1080p"), ("5", "Ultra-HD")]
    );
    assert_eq!(
        choices(&tested, "root_folder"),
        [("/movies", "/movies"), ("/movies-4k", "/movies-4k")]
    );
}

// The Plugin page flags a Quality whose profile this check stops offering.
#[tokio::test]
async fn testing_radarrs_settings_stops_offering_a_profile_removed_from_radarr() {
    let fixture = Fixture::start().await;
    fixture.radarr.remove_profile(4);

    let tested = fixture.test_settings(RADARR_KEY).await.unwrap();

    assert_eq!(
        choices(&tested, "quality_profile"),
        [("1", "Any"), ("5", "Ultra-HD")]
    );
}

#[tokio::test]
async fn testing_radarrs_settings_with_a_wrong_key_or_radarr_down_fails_in_plain_words() {
    let mut fixture = Fixture::start().await;

    let wrong_key = fixture.test_settings("not-the-key").await;
    fixture.radarr.go_down();
    let down = fixture.test_settings(RADARR_KEY).await;

    for (failed, message) in [
        (wrong_key, "Radarr refused the API key"),
        (down, "Radarr didn't answer"),
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
async fn approving_a_movie_request_adds_it_to_radarr_with_the_chosen_profile_and_folder_and_starts_a_search()
 {
    let fixture = Fixture::with_radarr_enabled().await;

    let approved = fixture.approve_movie(&fixture.member().await, HEAT).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert!(approved.quality.is_unset());
    assert!(approved.quality_used.is_unset());
    let added = json!({
        "tmdbId": HEAT,
        "title": "Heat",
        "qualityProfileId": 4,
        "rootFolderPath": "/movies",
        "monitored": true,
        "minimumAvailability": "released",
        "addOptions": { "searchForMovie": true },
    });
    assert_eq!(fixture.radarr.added(), [added]);
}

#[tokio::test]
async fn with_radarr_down_a_request_fails_in_plain_words_and_retrying_once_it_is_back_moves_it_to_processing()
 {
    let mut fixture = Fixture::with_radarr_enabled().await;
    fixture.radarr.go_down();

    let failed = fixture.approve_movie(&fixture.member().await, HEAT).await;
    fixture.radarr.come_back().await;
    let retried = fixture.retry(failed.id).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Radarr didn't answer")
    );
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(fixture.radarr.added().len(), 1);
}

#[tokio::test]
async fn a_movie_radarr_already_has_goes_to_processing() {
    let fixture = Fixture::with_radarr_enabled().await;

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    assert_eq!(approved.state, RequestState::Processing);
}

#[tokio::test]
async fn a_movie_radarr_cant_find_fails_with_radarrs_reason() {
    let fixture = Fixture::with_radarr_enabled().await;

    let failed = fixture.approve_movie(&fixture.member().await, DUNE).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Radarr said: A movie with this ID was not found.")
    );
}

/// The next event on `subscription`.
#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_event(subscription: &mut Subscription) -> event::Event {
    let next = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for an event")
        .unwrap()
        .expect("the stream ended")
        .to_owned_message()
        .message;
    let Some(subscribe_response::Message::Event(event)) = next else {
        panic!("not an event: {next:?}");
    };
    event.event.expect("an event of no kind")
}

/// The Request and its downloads in the next progress event on `subscription`.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_progress(subscription: &mut Subscription) -> (i64, Vec<Download>) {
    let event::Event::RequestProgress(progress) = next_event(subscription).await else {
        panic!("not progress");
    };
    let request = progress
        .request
        .into_option()
        .expect("progress of no Request");
    (request.id, request.downloads)
}

#[tokio::test]
async fn processing_movie_requests_show_radarrs_queue_as_it_advances_asked_once_per_poll() {
    let fixture = Fixture::with_radarr_enabled().await;
    fixture.radarr.set_queue("queue.json");
    let mut progress = fixture.subscribe_to_progress().await;
    let ada = fixture.member().await;

    let heat = fixture.approve_movie(&ada, HEAT).await;
    let first = next_progress(&mut progress).await;
    fixture.radarr.set_queue("queue-later.json");
    // Becoming Processing wakes the poll at once, rather than in a minute.
    let alien = fixture.approve_movie(&ada, ALIEN).await;
    let mut later = [
        next_progress(&mut progress).await,
        next_progress(&mut progress).await,
    ];
    later.sort_by_key(|(id, _)| *id);

    let heat_at = |percent, eta_seconds| Download {
        title: "Heat.1995.1080p.BluRay.x264".to_owned(),
        percent,
        size_bytes: Some(8_000_000_000),
        eta_seconds: Some(eta_seconds),
        status: DownloadStatus::DOWNLOAD_STATUS_DOWNLOADING.into(),
        ..Default::default()
    };
    let alien_blocked = Download {
        title: "Alien.1979.Directors.Cut.1080p".to_owned(),
        percent: 100.0,
        size_bytes: Some(4_000_000_000),
        status: DownloadStatus::DOWNLOAD_STATUS_NEEDS_ATTENTION.into(),
        attention_reason: Some(
            "Radarr said: Found matching movie via grab history, but release was matched to \
             movie by ID. Automatic import is not possible."
                .to_owned(),
        ),
        ..Default::default()
    };
    assert_eq!(first, (heat.id, vec![heat_at(25.0, 93_600)]));
    assert_eq!(
        later,
        [
            (heat.id, vec![heat_at(60.0, 2_700)]),
            (alien.id, vec![alien_blocked])
        ]
    );
    assert_eq!(fixture.radarr.queue_calls(), 2);
}

#[tokio::test]
async fn with_radarr_refusing_the_key_or_down_progress_fails_in_plain_words() {
    let mut fixture = Fixture::with_radarr_enabled().await;
    fixture.radarr.set_queue("queue.json");
    let heat = [HEAT.to_string()];

    let answered = fixture.progress(&heat).await;
    fixture.radarr.refuse_key();
    let refused = fixture.progress(&heat).await;
    fixture.radarr.go_down();
    let down = fixture.progress(&heat).await;

    assert_eq!(answered, Ok(vec![HEAT.to_string()]));
    assert_eq!(refused, Err("Radarr refused the API key".to_owned()));
    assert_eq!(down, Err("Radarr didn't answer".to_owned()));
}

#[tokio::test]
async fn with_a_quality_a_movie_is_added_to_radarr_at_its_profile() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await;

    fixture.approve_movie(&fixture.member().await, HEAT).await;

    let added = fixture.radarr.added();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn a_request_whose_profile_was_removed_from_radarr_fails_naming_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await;
    fixture.radarr.remove_profile(4);

    let failed = fixture.approve_movie(&fixture.member().await, HEAT).await;

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(
        failed.failure_reason.as_deref(),
        Some("Radarr has no quality profile with the id 4, which HD uses")
    );
    assert_eq!(fixture.radarr.added(), Vec::<serde_json::Value>::new());
}

#[tokio::test]
async fn a_movie_radarr_already_had_goes_to_processing_at_its_own_profile_though_the_asked_one_was_removed()
 {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await;
    fixture.radarr.remove_profile(4);

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    assert_eq!(approved.state, RequestState::Processing);
}

#[tokio::test]
async fn a_movie_radarr_already_had_at_a_profile_no_quality_offers_shows_radarrs_name_for_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture
        .replace_qualities(vec![quality(HD_1080P, "HD", true)])
        .await;

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    let used = &approved.quality_used;
    assert_eq!(
        (used.id, used.name.as_str(), used.profile_id.as_str()),
        (0, "Ultra-HD", ULTRA_HD)
    );
}
