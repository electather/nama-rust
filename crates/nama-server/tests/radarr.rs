//! The Radarr Installed Plugin, bundled in the Server, against a fake Radarr: its settings check
//! on the Plugins page, Movie Requests handed to it once Approved, and their download progress.

#[path = "support/fake_radarr.rs"]
mod fake_radarr;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient, ServerStream},
};
use fake_radarr::{API_KEY as RADARR_KEY, FakeRadarr};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveRequestRequest, CreateRequestRequest,
    CreateUserRequest, Download, DownloadStatus, EnablePluginRequest, EventServiceClient,
    EventType, ListPluginsRequest, PluginServiceClient, RefusalReason, Request,
    RequestServiceClient, RequestState, RetryRequestRequest, SessionDelivery, SignInRequest,
    SignInServiceClient, SubscribeRequest, SubscribeResponse, TestPluginSettingsRequest,
    TestPluginSettingsResponse, UpdatePluginSettingsRequest, event, setting_value::Value,
    subscribe_response,
};
use nama_server::{config::Config, plugin_settings, plugins::Registry};
use refused::assert_refused;
use serde_json::json;
use server::{PASSWORD, TestServer};

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
    radarr: FakeRadarr,
    _tmdb: FakeTmdb,
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
        let (server, db) = TestServer::start_with_db(Config::default(), plugins).await;
        // As on a Server's first start.
        plugin_settings::import(&db, &BTreeMap::new())
            .await
            .unwrap();
        let fixture = Self {
            server,
            radarr: FakeRadarr::start(&[ALIEN], &[DUNE]).await,
            _tmdb: tmdb,
        };
        // Listing is what reads the Installed Plugins, bundled ones too.
        fixture
            .plugins()
            .list_plugins(ListPluginsRequest::default())
            .await
            .unwrap();
        fixture
    }

    /// A Fixture where an Admin set Radarr up with the profile HD-1080p and the root folder
    /// `/movies`, and enabled it.
    async fn with_radarr_enabled() -> Self {
        let fixture = Self::start().await;
        let mut settings = fixture.radarr_settings(RADARR_KEY);
        settings.extend([("quality_profile", "4"), ("root_folder", "/movies")].map(text));
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

    /// The fake Radarr's URL and `api_key` as Radarr's settings.
    fn radarr_settings(&self, api_key: &str) -> Vec<(String, nama_proto::nama::v1::SettingValue)> {
        vec![
            text(("url", &self.radarr.url())),
            text(("api_key", api_key)),
        ]
    }

    /// Tests Radarr's settings with the fake Radarr's URL and `api_key`, as the Plugins page's
    /// "Test" does.
    async fn test_settings(
        &self,
        api_key: &str,
    ) -> Result<TestPluginSettingsResponse, ConnectError> {
        let request = TestPluginSettingsRequest {
            id: "radarr".to_owned(),
            settings: self.radarr_settings(api_key).into_iter().collect(),
            ..Default::default()
        };
        Ok(self
            .plugins()
            .test_plugin_settings(request)
            .await?
            .into_owned())
    }

    fn plugins(&self) -> PluginServiceClient<HttpClient> {
        PluginServiceClient::new(HttpClient::plaintext(), self.server.client_config())
    }

    fn requests(&self) -> RequestServiceClient<HttpClient> {
        RequestServiceClient::new(HttpClient::plaintext(), self.server.client_config())
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
    #[expect(
        clippy::unwrap_used,
        reason = "a test helper fails its test by panicking"
    )]
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
        let users = nama_proto::nama::v1::UserServiceClient::new(
            HttpClient::plaintext(),
            self.server.client_config(),
        );
        users
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

/// `(key, value)` as the Plugins page sends a text setting.
fn text((key, value): (&str, &str)) -> (String, nama_proto::nama::v1::SettingValue) {
    let value = nama_proto::nama::v1::SettingValue {
        value: Some(Value::TextValue(value.to_owned())),
        ..Default::default()
    };
    (key.to_owned(), value)
}

/// The `(value, label)` of each choice `tested` offers for the setting `key`.
fn choices<'a>(tested: &'a TestPluginSettingsResponse, key: &str) -> Vec<(&'a str, &'a str)> {
    tested.choices.get(key).map_or_else(Vec::new, |offered| {
        offered
            .choices
            .iter()
            .map(|choice| (choice.value.as_str(), choice.label.as_str()))
            .collect()
    })
}

#[tokio::test]
async fn testing_radarrs_settings_offers_its_quality_profiles_and_root_folders() {
    let fixture = Fixture::start().await;

    let tested = fixture.test_settings(RADARR_KEY).await.unwrap();

    assert_eq!(
        choices(&tested, "quality_profile"),
        [("1", "Any"), ("4", "HD-1080p")]
    );
    assert_eq!(
        choices(&tested, "root_folder"),
        [("/movies", "/movies"), ("/movies-4k", "/movies-4k")]
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

/// The Request and its downloads in the next progress event on `subscription`.
#[expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_progress(subscription: &mut Subscription) -> (i64, Vec<Download>) {
    let next = tokio::time::timeout(WAIT, subscription.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for progress")
        .unwrap()
        .expect("the stream ended")
        .to_owned_message()
        .message;
    let Some(subscribe_response::Message::Event(event)) = next else {
        panic!("not an event: {next:?}");
    };
    let Some(event::Event::RequestProgress(progress)) = event.event else {
        panic!("not progress: {event:?}");
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
