//! The Sonarr Installed Plugin, bundled in the Server, against a fake Sonarr: its settings check
//! on the Plugins page, Season Requests handed to it once Approved, and their download progress.

#[path = "support/fake_sonarr.rs"]
mod fake_sonarr;
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
use fake_sonarr::{API_KEY as SONARR_KEY, DARK_TVDB, FakeSonarr};
use nama_plugin_api::{
    fulfiller::Shows,
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY as TMDB_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    __buffa::view::SubscribeResponseView, ApproveFollowedShowRequest, ApproveRequestRequest,
    CancelFollowedShowRequest, CreateFollowedShowRequest, CreateSeasonRequestsRequest,
    CreateUserRequest, DeclineRequestRequest, Download, DownloadStatus, EnablePluginRequest,
    EventServiceClient, EventType, ExtensionPoint, FollowedShow, GetRequestRequest,
    ListDefaultQualitiesRequest, ListShowQualitiesRequest, LookUpShowRequest, Permission, Quality,
    RefusalReason, ReplaceQualitiesRequest, Request, RequestServiceClient, RequestState,
    RequestedQuality, RetryRequestRequest, SessionDelivery, SetDefaultQualityRequest,
    SetPermissionsRequest, SignInRequest, SignInServiceClient, SubscribeRequest, SubscribeResponse,
    UpdatePluginSettingsRequest, event, subscribe_response,
};
use nama_server::{config::Config, db::Database, plugins::Registry};
use refused::assert_refused;
use servarr::{ServarrFixture, choices, quality, text};
use server::{PASSWORD, TestServer};

/// Shows the fake TMDB knows, by TMDB id. Sonarr can find Dark, but not The Office.
const DARK: i64 = 70_523;
const THE_OFFICE: i64 = 2316;

/// Quality profiles the fake Sonarr offers, by id.
const HD_1080P: &str = "4";
const ULTRA_HD: &str = "5";

/// How long to wait for the Server to tell of progress.
const WAIT: Duration = Duration::from_secs(10);

type Subscription = ServerStream<hyper::body::Incoming, SubscribeResponseView<'static>>;

/// A Server with the TMDB Plugin enabled against a fake TMDB, beside a fake Sonarr that has no
/// Shows yet.
struct Fixture {
    server: TestServer,
    db: Arc<Database>,
    /// The Server's Plugins, to ask Sonarr's Fulfiller directly.
    plugins: Registry,
    sonarr: FakeSonarr,
    _tmdb: FakeTmdb,
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
        Self {
            server,
            db,
            plugins,
            sonarr: FakeSonarr::start().await,
            _tmdb: tmdb,
        }
    }

    /// A Fixture where an Admin set Sonarr up with the profile HD-1080p and the root folder `/tv`,
    /// and enabled it.
    async fn with_sonarr_enabled() -> Self {
        let fixture = Self::start().await;
        let mut settings = fixture.settings(SONARR_KEY);
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

    /// The Seasons `seasons` of the Show `show`, asked for by the Admin and so Approved at once, as
    /// their hand-offs left them.
    async fn ask_approved(&self, show: i64, seasons: &[u32]) -> Vec<Request> {
        ask(&self.requests(), show, seasons).await
    }

    /// The whole Show `show`, asked for by the Admin and so Approved at once. Returns the Followed
    /// Show's id and its Season Requests as their hand-offs left them.
    async fn follow_approved(&self, show: i64) -> (i64, Vec<Request>) {
        let (followed, requests) = follow(&self.requests(), show, None).await.unwrap();
        (followed.id, requests)
    }

    /// Approves the Followed Show `id` as the Admin, changing the Show's Quality to `quality` if
    /// given, and returns it as the approval left it.
    async fn approve_followed_at(
        &self,
        id: i64,
        quality: Option<i64>,
    ) -> Result<FollowedShow, ConnectError> {
        let approved = self
            .requests()
            .approve_followed_show(ApproveFollowedShowRequest {
                id,
                quality_id: quality,
                ..Default::default()
            })
            .await?;
        Ok(approved.into_owned().followed_show.into_option().unwrap())
    }

    /// Cancels the Followed Show `id` as the Admin.
    async fn cancel_followed(&self, id: i64) {
        self.requests()
            .cancel_followed_show(CancelFollowedShowRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
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
        self.approve_at(id, None).await.unwrap()
    }

    /// [`Self::approve`], changing its Quality to `quality` if given.
    async fn approve_at(&self, id: i64, quality: Option<i64>) -> Result<Request, ConnectError> {
        let approved = self
            .requests()
            .approve_request(ApproveRequestRequest {
                id,
                quality_id: quality,
                ..Default::default()
            })
            .await?;
        Ok(approved.into_owned().request.into_option().unwrap())
    }

    /// Declines the Request `id` as the Admin.
    async fn decline(&self, id: i64) {
        self.requests()
            .decline_request(DeclineRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
    }

    /// The Request `id`, as the Admin sees it.
    async fn request(&self, id: i64) -> Request {
        let got = self
            .requests()
            .get_request(GetRequestRequest {
                id,
                ..Default::default()
            })
            .await
            .unwrap();
        got.into_owned().request.into_option().unwrap()
    }

    /// The Quality of the Show `show`, as its page reads it.
    async fn show_quality(&self, show: i64) -> Option<RequestedQuality> {
        let looked_up = self
            .requests()
            .look_up_show(LookUpShowRequest {
                show_tmdb_id: show,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        looked_up.show.into_option().unwrap().quality.into_option()
    }

    /// Offers Sonarr's profiles HD-1080p as "HD", the default, and Ultra-HD as "4K", as the Admin,
    /// returning them as saved.
    async fn offer_hd_and_4k(&self) -> Vec<Quality> {
        self.replace_qualities(vec![
            quality(HD_1080P, "HD", true),
            quality(ULTRA_HD, "4K", false),
        ])
        .await
    }

    /// Replaces Sonarr's Qualities with `qualities` as the Admin, returning them as saved.
    async fn replace_qualities(&self, qualities: Vec<Quality>) -> Vec<Quality> {
        self.plugins()
            .replace_qualities(ReplaceQualitiesRequest {
                id: "sonarr".to_owned(),
                qualities,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .qualities
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
        self.member_with(None).await
    }

    /// A new User who may choose a Quality, as a client calling as them.
    async fn chooser(&self) -> RequestServiceClient<HttpClient> {
        let permissions = [Permission::Request, Permission::ChooseQuality];
        RequestServiceClient::new(
            HttpClient::plaintext(),
            self.member_with(Some(&permissions)).await,
        )
    }

    /// [`Self::member`], with `permissions` the Admin gives them in place of a new User's, if any.
    async fn member_with(&self, permissions: Option<&[Permission]>) -> ClientConfig {
        let users = nama_proto::nama::v1::UserServiceClient::new(
            HttpClient::plaintext(),
            self.server.client_config(),
        );
        let created = users
            .create_user(CreateUserRequest {
                name: "Ada".to_owned(),
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
    ask_at(client, show, seasons, None).await.unwrap()
}

/// [`ask`], at the Quality `quality` if given.
async fn ask_at(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
    seasons: &[u32],
    quality: Option<i64>,
) -> Result<Vec<Request>, ConnectError> {
    let asked = client
        .create_season_requests(CreateSeasonRequestsRequest {
            show_tmdb_id: show,
            season_numbers: seasons.to_vec(),
            quality,
            ..Default::default()
        })
        .await?;
    Ok(asked.into_owned().requests)
}

/// Asks for the whole Show `show` as `client`'s User, at the Quality `quality` if given, and
/// returns the Followed Show and its Season Requests as asking left them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn follow(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
    quality: Option<i64>,
) -> Result<(FollowedShow, Vec<Request>), ConnectError> {
    let followed = client
        .create_followed_show(CreateFollowedShowRequest {
            show_tmdb_id: show,
            quality,
            ..Default::default()
        })
        .await?
        .into_owned();
    Ok((
        followed.followed_show.into_option().unwrap(),
        followed.requests,
    ))
}

/// The one Season Request asking for the Season `season` of the Show `show` as `client`'s User
/// opens or joins, at the Quality `quality` if given.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask_one(
    client: &RequestServiceClient<HttpClient>,
    show: i64,
    season: u32,
    quality: Option<i64>,
) -> Request {
    ask_at(client, show, &[season], quality)
        .await
        .unwrap()
        .remove(0)
}

/// The id and name of `quality`, if a Request or Show has one.
fn named(quality: Option<&RequestedQuality>) -> Option<(i64, &str)> {
    quality.map(|quality| (quality.id, quality.name.as_str()))
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
async fn a_first_season_request_at_4k_has_sonarr_add_the_series_at_its_profile() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;

    let listed = chooser
        .list_show_qualities(ListShowQualitiesRequest::default())
        .await
        .unwrap();
    let asked = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    let approved = fixture.approve(asked.id).await;

    assert_eq!(listed.into_owned().qualities, offered);
    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(
        named(approved.quality.as_option()),
        Some((offered[1].id, "4K"))
    );
    assert_eq!(approved.quality_used, approved.quality);
    assert_eq!(fixture.sonarr.shows()[0]["qualityProfileId"], 5);
}

#[tokio::test]
async fn a_later_season_request_takes_the_shows_quality_and_refuses_another() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let first = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    fixture.approve(first.id).await;

    let shows = fixture.show_quality(DARK).await;
    let later = ask_one(&chooser, DARK, 2, None).await;
    let other = ask_at(&chooser, DARK, &[3], Some(offered[0].id)).await;

    assert_eq!(named(shows.as_ref()), Some((offered[1].id, "4K")));
    assert_eq!(
        named(later.quality.as_option()),
        Some((offered[1].id, "4K"))
    );
    let refusal = assert_refused(
        other,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn seasons_asked_for_before_either_is_approved_share_the_first_ones_quality() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;

    ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    let second = ask_one(&chooser, DARK, 2, None).await;

    assert_eq!(second.state, RequestState::Pending);
    assert_eq!(
        named(second.quality.as_option()),
        Some((offered[1].id, "4K"))
    );
}

#[tokio::test]
async fn once_every_earlier_season_request_is_declined_the_show_has_no_quality_and_takes_another() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let first = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;

    fixture.decline(first.id).await;
    let shows = fixture.show_quality(DARK).await;
    let after = ask_one(&chooser, DARK, 2, Some(offered[0].id)).await;

    assert_eq!(shows, None);
    assert_eq!(
        named(after.quality.as_option()),
        Some((offered[0].id, "HD"))
    );
}

#[tokio::test]
async fn before_any_season_is_handed_off_approving_one_at_another_quality_changes_the_shows() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let first = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    let second = ask_one(&chooser, DARK, 2, None).await;

    let approved = fixture
        .approve_at(second.id, Some(offered[0].id))
        .await
        .unwrap();

    assert_eq!(approved.state, RequestState::Processing);
    assert_eq!(fixture.sonarr.shows()[0]["qualityProfileId"], 4);
    let first = fixture.request(first.id).await;
    assert_eq!(
        named(first.quality.as_option()),
        Some((offered[0].id, "HD"))
    );
}

#[tokio::test]
async fn once_a_season_was_handed_off_approving_a_later_one_at_another_quality_is_refused() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let first = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    fixture.approve(first.id).await;
    let later = ask_one(&chooser, DARK, 2, None).await;

    let refused = fixture.approve_at(later.id, Some(offered[0].id)).await;
    let same = fixture.approve_at(later.id, Some(offered[1].id)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality_id");
    assert_eq!(same.unwrap().state, RequestState::Processing);
}

#[tokio::test]
async fn a_series_sonarr_already_had_at_another_profile_shows_that_profile() {
    let fixture = Fixture::with_sonarr_enabled().await;
    // Added at the setting's profile, HD-1080p, before the Admin offered any Quality.
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.finish_refreshes().await;
    fixture
        .replace_qualities(vec![quality(ULTRA_HD, "4K", true)])
        .await;

    let asked = fixture.ask_approved(DARK, &[2]).await.remove(0);
    let shows = fixture.show_quality(DARK).await;

    assert_eq!(asked.state, RequestState::Processing);
    assert_eq!(asked.quality.name, "4K");
    let used = &asked.quality_used;
    assert_eq!(
        (used.id, used.name.as_str(), used.profile_id.as_str()),
        (0, "HD-1080p", HD_1080P)
    );
    assert_eq!(shows.as_ref(), asked.quality_used.as_option());
}

#[tokio::test]
async fn once_sonarr_kept_its_profile_for_a_season_the_show_shows_it_over_an_earlier_pending_one() {
    let fixture = Fixture::with_sonarr_enabled().await;
    // Added at the setting's profile, HD-1080p, before the Admin offered any Quality.
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.finish_refreshes().await;
    fixture
        .replace_qualities(vec![quality(ULTRA_HD, "4K", true)])
        .await;
    let member = RequestServiceClient::new(HttpClient::plaintext(), fixture.member().await);
    let later = ask(&member, DARK, &[2, 3]).await.remove(1);

    let approved = fixture.approve(later.id).await;
    let shows = fixture.show_quality(DARK).await;

    assert_eq!(approved.quality_used.name, "HD-1080p");
    assert_eq!(shows.as_ref(), approved.quality_used.as_option());
}

#[tokio::test]
async fn approving_a_season_at_the_quality_sonarr_kept_leaves_handed_off_seasons_as_asked() {
    let fixture = Fixture::with_sonarr_enabled().await;
    // Added at the setting's profile, HD-1080p, before the Admin offered any Quality.
    fixture.ask_approved(DARK, &[1]).await;
    fixture.sonarr.finish_refreshes().await;
    let offered = fixture
        .replace_qualities(vec![
            quality(HD_1080P, "HD", false),
            quality(ULTRA_HD, "4K", true),
        ])
        .await;
    let member = RequestServiceClient::new(HttpClient::plaintext(), fixture.member().await);
    let asked = ask(&member, DARK, &[2, 3]).await;
    let handed_off = fixture.approve(asked[0].id).await;

    fixture
        .approve_at(asked[1].id, Some(offered[0].id))
        .await
        .unwrap();
    let handed_off_now = fixture.request(handed_off.id).await;

    assert_eq!(handed_off.quality_used.name, "HD");
    assert_eq!(handed_off_now.quality, handed_off.quality);
    assert_eq!(handed_off_now.quality_change, handed_off.quality_change);
}

#[tokio::test]
async fn a_members_shows_default_lists_in_their_settings_and_a_season_request_takes_it() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;

    chooser
        .set_default_quality(SetDefaultQualityRequest {
            plugin_id: "sonarr".to_owned(),
            quality: Some(offered[1].id),
            ..Default::default()
        })
        .await
        .unwrap();
    let defaults = chooser
        .list_default_qualities(ListDefaultQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .defaults;
    let listed = chooser
        .list_show_qualities(ListShowQualitiesRequest::default())
        .await
        .unwrap()
        .into_owned();
    let asked = ask_one(&chooser, DARK, 1, None).await;

    let fulfillers: Vec<_> = defaults
        .iter()
        .map(|default| default.fulfiller.as_known())
        .collect();
    assert_eq!(
        fulfillers,
        [Some(ExtensionPoint::EXTENSION_POINT_SHOW_FULFILLER)]
    );
    assert_eq!(defaults[0].quality, Some(offered[1].id));
    assert_eq!(listed.personal_default, Some(offered[1].id));
    assert_eq!(
        named(asked.quality.as_option()),
        Some((offered[1].id, "4K"))
    );
}

#[tokio::test]
async fn following_a_show_at_4k_opens_its_season_requests_at_4k_and_sonarr_adds_the_series_at_4k() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;

    let (followed, asked) = follow(&fixture.requests(), DARK, Some(offered[1].id))
        .await
        .unwrap();

    let four_k = Some((offered[1].id, "4K"));
    assert_eq!(named(followed.quality.as_option()), four_k);
    let asked_at: Vec<_> = asked
        .iter()
        .map(|request| named(request.quality.as_option()))
        .collect();
    assert_eq!(asked_at, [four_k; 3]);
    assert_eq!(states(&asked), [Some(RequestState::Processing); 3]);
    assert_eq!(fixture.sonarr.shows()[0]["qualityProfileId"], 5);
}

#[tokio::test]
async fn a_member_without_choose_quality_following_a_show_at_a_quality_is_refused() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let member = RequestServiceClient::new(HttpClient::plaintext(), fixture.member().await);

    let refused = follow(&member, DARK, Some(offered[0].id)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_ALLOWED,
    );
    assert_eq!(refusal.field, "quality");
}

#[tokio::test]
async fn a_request_manager_approving_a_followed_show_as_hd_has_its_season_requests_use_hd() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let (followed, asked) = follow(&chooser, DARK, Some(offered[1].id)).await.unwrap();

    let approved = fixture
        .approve_followed_at(followed.id, Some(offered[0].id))
        .await
        .unwrap();
    let season_1 = fixture.request(asked[0].id).await;

    let hd = Some((offered[0].id, "HD"));
    assert_eq!(named(approved.quality.as_option()), hd);
    assert_eq!(season_1.state, RequestState::Processing);
    assert_eq!(named(season_1.quality.as_option()), hd);
    assert_eq!(fixture.sonarr.shows()[0]["qualityProfileId"], 4);
}

#[tokio::test]
async fn following_a_show_with_a_quality_takes_it_and_refuses_another() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;

    let other = follow(&chooser, DARK, Some(offered[0].id)).await;
    let (followed, _) = follow(&chooser, DARK, None).await.unwrap();

    let refusal = assert_refused(
        other,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality");
    assert_eq!(
        named(followed.quality.as_option()),
        Some((offered[1].id, "4K"))
    );
}

#[tokio::test]
async fn once_a_season_was_handed_off_approving_a_followed_show_at_another_quality_is_refused() {
    let fixture = Fixture::with_sonarr_enabled().await;
    let offered = fixture.offer_hd_and_4k().await;
    let chooser = fixture.chooser().await;
    let first = ask_one(&chooser, DARK, 1, Some(offered[1].id)).await;
    fixture.approve(first.id).await;
    let (followed, _) = follow(&chooser, DARK, None).await.unwrap();

    let refused = fixture
        .approve_followed_at(followed.id, Some(offered[0].id))
        .await;
    let same = fixture
        .approve_followed_at(followed.id, Some(offered[1].id))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_QUALITY_NOT_OFFERED,
    );
    assert_eq!(refusal.field, "quality_id");
    assert_eq!(same.unwrap().state, RequestState::Approved);
}
