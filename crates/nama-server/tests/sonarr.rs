//! The Sonarr Installed Plugin, bundled in the Server, against a fake Sonarr: its settings check
//! on the Plugins page, and Season Requests handed to it once Approved.

#[path = "support/fake_sonarr.rs"]
mod fake_sonarr;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, sync::Arc};

use connectrpc::{
    ConnectError, ErrorCode,
    client::{ClientConfig, HttpClient},
};
use fake_sonarr::{API_KEY as SONARR_KEY, DARK_TVDB, FakeSonarr};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    ApproveRequestRequest, CreateSeasonRequestsRequest, CreateUserRequest, EnablePluginRequest,
    ListPluginsRequest, PluginServiceClient, RefusalReason, Request, RequestServiceClient,
    RequestState, RetryRequestRequest, SessionDelivery, SignInRequest, SignInServiceClient,
    TestPluginSettingsRequest, TestPluginSettingsResponse, UpdatePluginSettingsRequest,
    setting_value::Value,
};
use nama_server::{config::Config, db::Database, plugin_settings, plugins::Registry};
use refused::assert_refused;
use server::{PASSWORD, TestServer};

/// Shows the fake TMDB knows, by TMDB id. Sonarr can find Dark, but not The Office.
const DARK: i64 = 70_523;
const THE_OFFICE: i64 = 2316;

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Sonarr that has no
/// Shows yet.
struct Fixture {
    server: TestServer,
    db: Arc<Database>,
    sonarr: FakeSonarr,
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
            db,
            sonarr: FakeSonarr::start().await,
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

    /// A Fixture where an Admin set Sonarr up with the profile HD-1080p and the root folder `/tv`,
    /// and enabled it.
    async fn with_sonarr_enabled() -> Self {
        let fixture = Self::start().await;
        let mut settings = fixture.sonarr_settings(SONARR_KEY);
        settings.extend([("quality_profile", "4"), ("root_folder", "/tv")].map(text));
        fixture
            .plugins()
            .update_plugin_settings(UpdatePluginSettingsRequest {
                id: "sonarr".to_owned(),
                settings: settings.into_iter().collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
            .plugins()
            .enable_plugin(EnablePluginRequest {
                id: "sonarr".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        fixture
    }

    /// The fake Sonarr's URL and `api_key` as Sonarr's settings.
    fn sonarr_settings(&self, api_key: &str) -> Vec<(String, nama_proto::nama::v1::SettingValue)> {
        vec![
            text(("url", &self.sonarr.url())),
            text(("api_key", api_key)),
        ]
    }

    /// Tests Sonarr's settings with the fake Sonarr's URL and `api_key`, as the Plugins page's
    /// "Test" does.
    async fn test_settings(
        &self,
        api_key: &str,
    ) -> Result<TestPluginSettingsResponse, ConnectError> {
        let request = TestPluginSettingsRequest {
            id: "sonarr".to_owned(),
            settings: self.sonarr_settings(api_key).into_iter().collect(),
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

    /// The Seasons `seasons` of the Show `show`, asked for by the Admin and so Approved at once, as
    /// their hand-offs left them.
    async fn ask_approved(&self, show: i64, seasons: &[u32]) -> Vec<Request> {
        ask(&self.requests(), show, seasons).await
    }

    /// The Season `season` of the Show `show`, asked for by a new User and so Pending.
    async fn ask_pending(&self, show: i64, season: u32) -> Request {
        let member = RequestServiceClient::new(HttpClient::plaintext(), self.member().await);
        let asked = ask(&member, show, &[season]).await.remove(0);
        assert_eq!(asked.state, RequestState::Pending);
        asked
    }

    /// Approves the Request `id` as the Admin, and returns it as the approval left it.
    async fn approve(&self, id: i64) -> Request {
        let approved = self
            .requests()
            .approve_request(ApproveRequestRequest {
                id,
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

    /// Forgets every Season Request's Show TVDB id, as for a Show the metadata source has none
    /// for.
    async fn forget_show_tvdb_ids(&self) {
        self.db
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

/// Asks for the Seasons `seasons` of the Show `show` as `client`'s User, and returns the Requests
/// as asking left them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
    seasons: &[u32],
) -> Vec<Request> {
    client
        .create_season_requests(CreateSeasonRequestsRequest {
            show_tmdb_id: show,
            season_numbers: seasons.to_vec(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .requests
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
        [("1", "Any"), ("4", "HD-1080p")]
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

    assert_eq!(states(&asked), [Some(RequestState::Processing)]);
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 2, 3]);
    assert_eq!(fixture.sonarr.searches().last(), Some(&(DARK_TVDB, 2)));
}

#[tokio::test]
async fn a_show_another_hand_off_added_meanwhile_gets_the_season_monitored_rather_than_failing() {
    let fixture = Fixture::with_sonarr_enabled().await;
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.answer_next_lookup_stale();

    let asked = fixture.ask_approved(DARK, &[3]).await;

    assert_eq!(states(&asked), [Some(RequestState::Processing)]);
    assert_eq!(fixture.sonarr.shows().len(), 1);
    assert_eq!(fixture.sonarr.monitored_seasons(DARK_TVDB), [1, 3]);
}

#[tokio::test]
async fn with_sonarr_down_a_request_fails_in_plain_words_and_retrying_once_it_is_back_moves_it_to_processing()
 {
    let mut fixture = Fixture::with_sonarr_enabled().await;
    fixture.sonarr.go_down();

    let failed = fixture.ask_approved(DARK, &[1]).await.remove(0);
    fixture.sonarr.come_back().await;
    let retried = fixture.retry(failed.id).await;

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
    let pending = fixture.ask_pending(DARK, 1).await;
    fixture.forget_show_tvdb_ids().await;

    let failed = fixture.approve(pending.id).await;

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
