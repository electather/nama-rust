//! The Radarr Installed Plugin, bundled in the Server, against a fake Radarr: its settings check
//! on the Plugins page, Movie Requests handed to it once Approved, and their download progress.

#[path = "support/fake_radarr.rs"]
mod fake_radarr;
#[path = "support/refused.rs"]
mod refused;
#[path = "support/servarr.rs"]
mod servarr;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use connectrpc::{
    ConnectError, ErrorCode,
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
    CreateUserRequest, DefaultQuality, Download, DownloadStatus, EnablePluginRequest,
    EventServiceClient, EventType, ExtensionPoint, ListDefaultQualitiesRequest,
    ListMovieQualitiesRequest, ListOfferedQualitiesRequest, ListPluginsRequest,
    ListQualitiesRequest, Permission, Quality, RefusalReason, ReplaceQualitiesRequest, Request,
    RequestServiceClient, RequestState, RetryRequestRequest, SessionDelivery,
    SetDefaultQualityRequest, SetPermissionsRequest, SignInRequest, SignInServiceClient,
    SubscribeRequest, SubscribeResponse, TitleKind, UpdatePluginSettingsRequest, event,
    subscribe_response,
};
use nama_server::{config::Config, plugin_settings, plugins::Registry};
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
        let (server, db) = TestServer::start_with_db(Config::default(), plugins.clone()).await;
        // As on a Server's first start.
        plugin_settings::import(&db, &BTreeMap::new())
            .await
            .unwrap();
        let fixture = Self {
            server,
            plugins,
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
        let asked = ask(member, tmdb_id, None).await.unwrap();
        assert_eq!(asked.state, RequestState::Pending);
        self.approve(asked.id, None).await.unwrap()
    }

    /// Approves the Request `id` as the Admin, at the Quality `quality_id` if given, and returns
    /// it as the approval left it.
    async fn approve(&self, id: i64, quality_id: Option<i64>) -> Result<Request, ConnectError> {
        let approved = self
            .requests()
            .approve_request(ApproveRequestRequest {
                id,
                quality_id,
                ..Default::default()
            })
            .await?;
        Ok(approved.into_owned().request.into_option().unwrap())
    }

    /// Replaces Radarr's Qualities with `qualities` as the Admin, returning them as saved.
    async fn replace_qualities(
        &self,
        qualities: Vec<Quality>,
    ) -> Result<Vec<Quality>, ConnectError> {
        let replaced = self
            .plugins()
            .replace_qualities(ReplaceQualitiesRequest {
                id: "radarr".to_owned(),
                qualities,
                ..Default::default()
            })
            .await?;
        Ok(replaced.into_owned().qualities)
    }

    /// Radarr's Qualities, as the Admin reads them.
    async fn qualities(&self) -> Vec<Quality> {
        self.plugins()
            .list_qualities(ListQualitiesRequest {
                id: "radarr".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .qualities
    }

    /// Retries the Request `id` as the Admin, at the Quality `quality_id` if given, and returns
    /// it as the retry left it.
    async fn retry(&self, id: i64, quality_id: Option<i64>) -> Result<Request, ConnectError> {
        let retried = self
            .requests()
            .retry_request(RetryRequestRequest {
                id,
                quality_id,
                ..Default::default()
            })
            .await?;
        Ok(retried.into_owned().request.into_option().unwrap())
    }

    /// Subscribes as the Admin to the progress of every Request.
    async fn subscribe_to_progress(&self) -> Subscription {
        self.subscribe_to(EventType::EVENT_TYPE_REQUEST_PROGRESS)
            .await
    }

    /// Subscribes as the Admin to the events of type `only` about every Request.
    async fn subscribe_to(&self, only: EventType) -> Subscription {
        subscribe(self.server.client_config(), only).await
    }

    /// Creates a User with the Permissions a new User gets, signs them in, and returns client
    /// settings that call as them.
    async fn member(&self) -> ClientConfig {
        self.member_with(None).await
    }

    /// [`Self::member`], with `permissions` the Admin gives them in place of a new User's, if any.
    async fn member_with(&self, permissions: Option<&[Permission]>) -> ClientConfig {
        self.member_named("Ada", permissions).await
    }

    /// [`Self::member_with`], named `name`.
    async fn member_named(&self, name: &str, permissions: Option<&[Permission]>) -> ClientConfig {
        let users = nama_proto::nama::v1::UserServiceClient::new(
            HttpClient::plaintext(),
            self.server.client_config(),
        );
        let created = users
            .create_user(CreateUserRequest {
                name: name.to_owned(),
                password: PASSWORD.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
        if let Some(permissions) = permissions {
            users
                .set_permissions(SetPermissionsRequest {
                    user_id: created.into_owned().id,
                    permissions: permissions.iter().map(|&p| p.into()).collect(),
                    ..Default::default()
                })
                .await
                .unwrap();
        }
        let signed_in = SignInServiceClient::new(
            HttpClient::plaintext(),
            self.server.anonymous_client_config(),
        )
        .sign_in(SignInRequest {
            name: name.to_owned(),
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

/// Subscribes as `member` to the events of type `only` they may see.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn subscribe(member: ClientConfig, only: EventType) -> Subscription {
    let only = SubscribeRequest {
        types: vec![only.into()],
        ..Default::default()
    };
    EventServiceClient::new(HttpClient::plaintext(), member)
        .subscribe(only)
        .await
        .unwrap()
}

/// Asks for the Movie `tmdb_id` as `member`, at the Quality `quality` if given.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask(
    member: &ClientConfig,
    tmdb_id: i64,
    quality: Option<i64>,
) -> Result<Request, ConnectError> {
    let asked = RequestServiceClient::new(HttpClient::plaintext(), member.clone())
        .create_request(CreateRequestRequest {
            movie_tmdb_id: tmdb_id,
            quality,
            ..Default::default()
        })
        .await?;
    Ok(asked.into_owned().request.into_option().unwrap())
}

/// Sets `member`'s default Quality for Radarr to `quality`, or clears it.
async fn set_default(member: &ClientConfig, quality: Option<i64>) -> Result<(), ConnectError> {
    RequestServiceClient::new(HttpClient::plaintext(), member.clone())
        .set_default_quality(SetDefaultQualityRequest {
            plugin_id: "radarr".to_owned(),
            quality,
            ..Default::default()
        })
        .await?;
    Ok(())
}

/// `member`'s default Qualities, as their settings read them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn defaults(member: &ClientConfig) -> Vec<DefaultQuality> {
    RequestServiceClient::new(HttpClient::plaintext(), member.clone())
        .list_default_qualities(ListDefaultQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .defaults
}

/// The id of `member`'s default Quality for Movies, as the Request dialog reads it.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn personal_default(member: &ClientConfig) -> Option<i64> {
    RequestServiceClient::new(HttpClient::plaintext(), member.clone())
        .list_movie_qualities(ListMovieQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .personal_default
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
    let retried = fixture.retry(failed.id, None).await.unwrap();

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

/// The Request as the next state change event on `subscription` has it.
#[expect(
    clippy::expect_used,
    clippy::panic,
    reason = "a test helper fails its test by panicking"
)]
async fn next_state_change(subscription: &mut Subscription) -> Request {
    let event::Event::RequestStateChanged(changed) = next_event(subscription).await else {
        panic!("not a state change");
    };
    changed
        .request
        .into_option()
        .expect("a change of no Request")
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
async fn a_movie_request_goes_to_radarr_at_the_default_quality_an_admin_offers() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();

    let approved = fixture.approve_movie(&fixture.member().await, HEAT).await;

    assert_eq!(fixture.qualities().await, offered);
    let hd = &offered[0];
    assert_eq!(
        (approved.quality.id, approved.quality.name.as_str()),
        (hd.id, "HD")
    );
    assert_eq!(approved.quality.profile_id, HD_1080P);
    assert!(approved.quality_change.is_unset());
    assert_eq!(approved.quality_used, approved.quality);
    let added = fixture.radarr.added();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn a_request_keeps_its_quality_and_retries_with_its_profile_after_it_is_renamed_and_removed()
{
    let mut fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    fixture.radarr.go_down();
    let failed = fixture.approve_movie(&fixture.member().await, HEAT).await;
    fixture.radarr.come_back().await;

    let mut renamed = offered.clone();
    renamed[0].name = "Full HD".to_owned();
    fixture.replace_qualities(renamed).await.unwrap();
    let mut only_4k = offered[1].clone();
    only_4k.is_default = true;
    let kept = fixture
        .replace_qualities(vec![only_4k.clone()])
        .await
        .unwrap();
    let retried = fixture.retry(failed.id, None).await.unwrap();

    assert_eq!(kept, [only_4k]);
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(retried.quality.name, "HD");
    assert!(retried.quality_change.is_unset());
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
}

/// "HD" and "4K" as [`hd_and_4k`] offers them, but with 4K the default.
fn hd_and_default_4k() -> Vec<Quality> {
    vec![
        quality(HD_1080P, "HD", false),
        quality(ULTRA_HD, "4K", true),
    ]
}

#[tokio::test]
async fn a_request_manager_approving_a_4k_request_as_hd_has_radarr_asked_for_hd() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture
        .replace_qualities(hd_and_default_4k())
        .await
        .unwrap();
    let asked = ask(&fixture.member().await, HEAT, None).await.unwrap();
    let mut approvals = fixture
        .subscribe_to(EventType::EVENT_TYPE_REQUEST_STATE_CHANGED)
        .await;

    let offered_to_managers = fixture
        .requests()
        .list_offered_qualities(ListOfferedQualitiesRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .qualities;
    let approved = fixture
        .approve(asked.id, Some(offered[0].id))
        .await
        .unwrap();

    assert_eq!(offered_to_managers, offered);
    assert_eq!(asked.quality.name, "4K");
    assert_eq!(
        (
            approved.quality.name.as_str(),
            approved.quality.profile_id.as_str()
        ),
        ("HD", HD_1080P)
    );
    let change = &approved.quality_change;
    assert_eq!(change.changed_by.as_deref(), Some("Admin"));
    assert_eq!(change.previous_name.as_deref(), Some("4K"));
    assert!(change.changed_at > 0);
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
    let told = next_state_change(&mut approvals).await;
    assert_eq!(told.state, RequestState::Approved);
    assert_eq!(told.quality_change, approved.quality_change);
}

#[tokio::test]
async fn retrying_a_failed_request_at_another_quality_hands_it_off_at_that_one_keeping_only_that_change()
 {
    let mut fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let asked = ask(&fixture.member().await, HEAT, None).await.unwrap();
    fixture.radarr.go_down();
    let failed = fixture
        .approve(asked.id, Some(offered[1].id))
        .await
        .unwrap();
    fixture.radarr.come_back().await;

    let retried = fixture.retry(failed.id, Some(offered[0].id)).await.unwrap();

    assert_eq!(failed.state, RequestState::Failed);
    assert_eq!(failed.quality_change.previous_name.as_deref(), Some("HD"));
    assert_eq!(retried.state, RequestState::Processing);
    assert_eq!(retried.quality.name, "HD");
    assert_eq!(retried.quality_change.previous_name.as_deref(), Some("4K"));
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn approving_or_retrying_at_a_quality_not_on_the_list_is_refused() {
    let mut fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let member = fixture.member().await;
    let pending = ask(&member, HEAT, None).await.unwrap();
    fixture.radarr.go_down();
    let failed = fixture.approve_movie(&member, ALIEN).await;
    let unknown = Some(999);

    let approving = fixture.approve(pending.id, unknown).await;
    let retrying = fixture.retry(failed.id, unknown).await;

    for refused in [approving, retrying] {
        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
        );
        assert_eq!(refusal.field, "quality_id");
        assert_eq!(
            refusal.params.get("qualityId").map(String::as_str),
            Some("999")
        );
    }
    let still = fixture.approve(pending.id, None).await.unwrap();
    assert_eq!(still.quality.name, "HD");
}

#[tokio::test]
async fn a_request_whose_profile_was_removed_from_radarr_fails_naming_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await.unwrap();
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
async fn a_movie_radarr_already_had_at_another_quality_shows_that_quality() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(approved.quality.name, "HD");
    let used = &approved.quality_used;
    assert_eq!(
        (used.id, used.name.as_str(), used.profile_id.as_str()),
        (offered[1].id, "4K", ULTRA_HD)
    );
}

#[tokio::test]
async fn a_movie_radarr_already_had_goes_to_processing_at_its_own_profile_though_the_asked_one_was_removed()
 {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture.replace_qualities(hd_and_4k()).await.unwrap();
    fixture.radarr.remove_profile(4);

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(approved.quality_used.name, "4K");
}

#[tokio::test]
async fn a_movie_radarr_already_had_at_a_profile_no_quality_offers_shows_radarrs_name_for_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    fixture
        .replace_qualities(vec![quality(HD_1080P, "HD", true)])
        .await
        .unwrap();

    let approved = fixture.approve_movie(&fixture.member().await, ALIEN).await;

    let used = &approved.quality_used;
    assert_eq!(
        (used.id, used.name.as_str(), used.profile_id.as_str()),
        (0, "Ultra-HD", ULTRA_HD)
    );
}

#[tokio::test]
async fn saving_a_quality_whose_profile_radarr_doesnt_offer_is_refused_naming_it() {
    let fixture = Fixture::with_radarr_enabled().await;

    let refused = fixture
        .replace_qualities(vec![quality(ANY, "Any", true), quality("99", "8K", false)])
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_CHOICE_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "qualities[1].profile_id");
    assert_eq!(fixture.qualities().await, []);
}

#[tokio::test]
async fn saving_qualities_while_radarr_is_down_is_refused() {
    let mut fixture = Fixture::with_radarr_enabled().await;
    fixture.radarr.go_down();

    let refused = fixture.replace_qualities(hd_and_4k()).await;

    assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_SETTINGS_CHECK_FAILED,
    );
}

#[tokio::test]
async fn repeated_profiles_or_names_and_lists_without_one_default_are_refused() {
    let fixture = Fixture::with_radarr_enabled().await;
    let cases = [
        (
            vec![
                quality(HD_1080P, "HD", true),
                quality(HD_1080P, "Full HD", false),
            ],
            RefusalReason::REFUSAL_REASON_QUALITY_REPEATED,
            "qualities[1].profile_id",
        ),
        (
            vec![
                quality(HD_1080P, "HD", true),
                quality(ULTRA_HD, "hd", false),
            ],
            RefusalReason::REFUSAL_REASON_QUALITY_REPEATED,
            "qualities[1].name",
        ),
        (
            vec![
                quality(HD_1080P, "HD", false),
                quality(ULTRA_HD, "4K", false),
            ],
            RefusalReason::REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED,
            "qualities",
        ),
        (
            vec![quality(HD_1080P, "HD", true), quality(ULTRA_HD, "4K", true)],
            RefusalReason::REFUSAL_REASON_DEFAULT_QUALITY_REQUIRED,
            "qualities",
        ),
    ];

    for (qualities, reason, field) in cases {
        let refused = fixture.replace_qualities(qualities).await;

        let refusal = assert_refused(refused, ErrorCode::InvalidArgument, reason);
        assert_eq!(refusal.field, field);
    }
    assert_eq!(fixture.qualities().await, []);
}

#[tokio::test]
async fn a_quality_id_given_twice_is_kept_once_and_the_other_quality_gets_a_new_one() {
    let fixture = Fixture::with_radarr_enabled().await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let mut twice = hd_and_4k();
    twice[0].id = offered[0].id;
    twice[1].id = offered[0].id;

    let saved = fixture.replace_qualities(twice).await.unwrap();

    assert_eq!(saved[0].id, offered[0].id);
    assert_ne!(saved[1].id, offered[0].id);
}

#[tokio::test]
async fn a_member_given_choose_quality_picks_4k_and_radarr_adds_the_movie_at_its_profile() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let chooser = fixture
        .member_with(Some(&[Permission::Request, Permission::ChooseQuality]))
        .await;

    let listed = RequestServiceClient::new(HttpClient::plaintext(), chooser.clone())
        .list_movie_qualities(ListMovieQualitiesRequest::default())
        .await
        .unwrap();
    let asked = ask(&chooser, HEAT, Some(offered[1].id)).await.unwrap();
    let approved = fixture.approve(asked.id, None).await.unwrap();

    assert_eq!(listed.into_owned().qualities, offered);
    assert_eq!(approved.quality.name, "4K");
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(5));
}

#[tokio::test]
async fn a_quality_not_on_the_list_or_with_no_list_is_refused_naming_it() {
    let fixture = Fixture::with_radarr_enabled().await;
    let chooser = fixture
        .member_with(Some(&[Permission::Request, Permission::ChooseQuality]))
        .await;
    let without_a_list = ask(&chooser, HEAT, Some(1)).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let not_offered = offered.iter().map(|quality| quality.id).max().unwrap() + 1;

    let off_the_list = ask(&chooser, HEAT, Some(not_offered)).await;

    for refused in [without_a_list, off_the_list] {
        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
        );
        assert_eq!(refusal.field, "quality");
    }
}

#[tokio::test]
async fn a_member_without_choose_quality_naming_even_the_default_is_refused() {
    let fixture = Fixture::with_radarr_enabled().await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();

    let refused = ask(&fixture.member().await, HEAT, Some(offered[0].id)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_ALLOWED,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn a_member_whose_default_is_4k_gets_it_without_picking_and_picking_hd_overrides_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let chooser = fixture
        .member_with(Some(&[Permission::Request, Permission::ChooseQuality]))
        .await;
    let (hd, four_k) = (offered[0].id, offered[1].id);

    set_default(&chooser, Some(four_k)).await.unwrap();
    let defaulted = fixture
        .approve(ask(&chooser, HEAT, None).await.unwrap().id, None)
        .await
        .unwrap();
    let picked = ask(&chooser, ALIEN, Some(hd)).await.unwrap();

    assert_eq!(personal_default(&chooser).await, Some(four_k));
    assert_eq!(defaulted.quality.name, "4K");
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(5));
    assert_eq!(picked.quality.name, "HD");
}

#[tokio::test]
async fn after_the_admin_removes_a_members_default_they_get_the_fulfillers_and_settings_flag_it() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let chooser = fixture
        .member_with(Some(&[Permission::Request, Permission::ChooseQuality]))
        .await;
    set_default(&chooser, Some(offered[1].id)).await.unwrap();

    let kept = fixture
        .replace_qualities(vec![offered[0].clone()])
        .await
        .unwrap();
    let approved = fixture
        .approve(ask(&chooser, HEAT, None).await.unwrap().id, None)
        .await
        .unwrap();

    assert_eq!(approved.quality.name, "HD");
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
    assert_eq!(personal_default(&chooser).await, None);
    let defaults = defaults(&chooser).await;
    assert_eq!(defaults.len(), 1);
    assert_eq!(defaults[0].plugin_id, "radarr");
    assert_eq!(
        defaults[0].fulfiller,
        ExtensionPoint::EXTENSION_POINT_MOVIE_FULFILLER
    );
    assert_eq!(defaults[0].qualities, kept);
    let stale = defaults[0].quality;
    assert_eq!(stale, Some(offered[1].id));
    assert!(!kept.iter().any(|quality| Some(quality.id) == stale));
}

#[tokio::test]
async fn a_member_without_choose_quality_gets_the_fulfillers_default_whatever_theirs_is() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let member = fixture.member().await;

    set_default(&member, Some(offered[1].id)).await.unwrap();
    let approved = fixture
        .approve(ask(&member, HEAT, None).await.unwrap().id, None)
        .await
        .unwrap();

    assert_eq!(personal_default(&member).await, None);
    assert_eq!(approved.quality.name, "HD");
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn a_default_not_on_the_list_is_refused_naming_it_and_a_cleared_one_is_unset() {
    let fixture = Fixture::with_radarr_enabled().await;
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let chooser = fixture
        .member_with(Some(&[Permission::Request, Permission::ChooseQuality]))
        .await;
    let not_offered = offered.iter().map(|quality| quality.id).max().unwrap() + 1;

    let refused = set_default(&chooser, Some(not_offered)).await;
    set_default(&chooser, Some(offered[1].id)).await.unwrap();
    set_default(&chooser, None).await.unwrap();

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality");
    assert_eq!(defaults(&chooser).await[0].quality, None);
}

/// "HD" (the default) and "4K", and Alice, who may choose and opened a Pending Request for Heat at
/// 4K.
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "a test helper fails its test by panicking"
)]
async fn alices_pending_4k_request(fixture: &Fixture) -> (Vec<Quality>, ClientConfig) {
    let offered = fixture.replace_qualities(hd_and_4k()).await.unwrap();
    let alice = fixture
        .member_named(
            "Alice",
            Some(&[Permission::Request, Permission::ChooseQuality]),
        )
        .await;
    let asked = ask(&alice, HEAT, Some(offered[1].id)).await.unwrap();
    assert_eq!(asked.state, RequestState::Pending);
    (offered, alice)
}

#[tokio::test]
async fn an_auto_approved_joiner_picking_hd_approves_a_pending_4k_request_at_hd_and_alice_is_told()
{
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let (offered, alice) = alices_pending_4k_request(&fixture).await;
    let bob = fixture
        .member_named(
            "Bob",
            Some(&[
                Permission::Request,
                Permission::AutoApproveMovie,
                Permission::ChooseQuality,
            ]),
        )
        .await;
    let mut approvals = subscribe(alice, EventType::EVENT_TYPE_REQUEST_STATE_CHANGED).await;

    ask(&bob, HEAT, Some(offered[0].id)).await.unwrap();

    let told = next_state_change(&mut approvals).await;
    assert_eq!(told.state, RequestState::Approved);
    assert_eq!(told.quality.name, "HD");
    let change = &told.quality_change;
    assert_eq!(change.changed_by.as_deref(), Some("Bob"));
    assert_eq!(change.previous_name.as_deref(), Some("4K"));
    assert!(change.changed_at > 0);
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn an_auto_approved_joiner_choosing_nothing_approves_a_pending_request_at_its_quality() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    alices_pending_4k_request(&fixture).await;
    let bob = fixture
        .member_named(
            "Bob",
            Some(&[Permission::Request, Permission::AutoApproveMovie]),
        )
        .await;

    let joined = ask(&bob, HEAT, None).await.unwrap();

    assert_eq!(joined.state, RequestState::Processing);
    assert_eq!(joined.quality.name, "4K");
    assert!(joined.quality_change.is_unset());
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(5));
}

#[tokio::test]
async fn an_auto_approved_joiners_personal_default_counts_as_their_choice() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let (offered, _) = alices_pending_4k_request(&fixture).await;
    let bob = fixture
        .member_named(
            "Bob",
            Some(&[
                Permission::Request,
                Permission::AutoApproveMovie,
                Permission::ChooseQuality,
            ]),
        )
        .await;
    // HD is the Fulfiller's default too: Bob picking it still counts.
    set_default(&bob, Some(offered[0].id)).await.unwrap();

    let joined = ask(&bob, HEAT, None).await.unwrap();

    assert_eq!(joined.quality.name, "HD");
    assert_eq!(joined.quality_change.changed_by.as_deref(), Some("Bob"));
    assert_eq!(fixture.radarr.added()[0]["qualityProfileId"], json!(4));
}

#[tokio::test]
async fn a_joiner_who_isnt_auto_approved_leaves_the_quality_as_it_is() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let (offered, _) = alices_pending_4k_request(&fixture).await;
    let bob = fixture
        .member_named(
            "Bob",
            Some(&[Permission::Request, Permission::ChooseQuality]),
        )
        .await;

    let joined = ask(&bob, HEAT, Some(offered[0].id)).await.unwrap();

    assert_eq!(joined.state, RequestState::Pending);
    assert_eq!(joined.quality.name, "4K");
    assert!(joined.quality_change.is_unset());
}

#[tokio::test]
async fn an_auto_approved_joiners_default_no_longer_offered_isnt_their_choice() {
    let fixture = Fixture::with_radarr_enabled_at(ANY).await;
    let (offered, _) = alices_pending_4k_request(&fixture).await;
    let mut with_any = offered.clone();
    with_any.push(quality(ANY, "Any", false));
    let with_any = fixture.replace_qualities(with_any).await.unwrap();
    let bob = fixture
        .member_named(
            "Bob",
            Some(&[
                Permission::Request,
                Permission::AutoApproveMovie,
                Permission::ChooseQuality,
            ]),
        )
        .await;
    set_default(&bob, Some(with_any[2].id)).await.unwrap();
    fixture.replace_qualities(offered).await.unwrap();

    let joined = ask(&bob, HEAT, None).await.unwrap();

    assert_eq!(joined.quality.name, "4K");
    assert!(joined.quality_change.is_unset());
}
