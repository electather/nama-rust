//! Requesting Movies through the generated `RequestService` client, over a real socket, with the
//! TMDB Plugin enabled against a fake TMDB.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "the harness's plain `start` has no use here")]
#[path = "support/server.rs"]
mod server;

use std::{collections::BTreeMap, fs, sync::Arc, time::Duration};

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_plugin_api::settings::{SettingValue, Settings};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateLibraryRequest, CreateRequestRequest, CreateUserRequest,
    EventServiceClient, EventType, GetRequestRequest, LibraryKind, LibraryServiceClient,
    ListMoviesRequest, ListRequestsRequest, Permission, RefusalReason, Request,
    RequestServiceClient, RequestState, ScanLibraryRequest, SessionDelivery, SetPermissionsRequest,
    SignInRequest, SignInServiceClient, SubscribeRequest, SubscribeResponse, UserServiceClient,
    event, subscribe_response,
};
use nama_server::{config::Config, plugins::Registry};
use refused::assert_refused;
use server::{PASSWORD, TestServer};

/// Movies the fake TMDB knows.
const ALIEN: i64 = 348;
const HEAT: i64 = 949;
const DUNE: i64 = 438_631;

const WAIT: Duration = Duration::from_secs(10);

/// What a Member gets: `REQUEST` and `VIEW_REQUESTS`, as a new User does.
const MEMBER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];

/// A Server with the TMDB Plugin enabled against a fake TMDB, and an Admin.
struct Fixture {
    server: TestServer,
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
            SettingValue::Text(API_KEY.to_owned()),
        )]));
        plugins
            .enable_with_settings(Arc::new(Tmdb::with_endpoints(tmdb.endpoints())), settings)
            .unwrap();
        let server = TestServer::start_with_db(Config::default(), plugins)
            .await
            .0;
        Self {
            server,
            _tmdb: tmdb,
        }
    }

    /// Creates a User named `name` with `permissions`, signs them in, and returns their ID and
    /// client settings that call as them.
    async fn user(&self, name: &str, permissions: &[Permission]) -> (i64, ClientConfig) {
        let as_admin = UserServiceClient::new(HttpClient::plaintext(), self.server.client_config());
        let id = as_admin
            .create_user(CreateUserRequest {
                name: name.to_owned(),
                password: PASSWORD.to_owned(),
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .id;
        as_admin
            .set_permissions(SetPermissionsRequest {
                user_id: id,
                permissions: permissions.iter().map(|&p| p.into()).collect(),
                ..Default::default()
            })
            .await
            .unwrap();
        let token = SignInServiceClient::new(
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
        .unwrap()
        .into_owned()
        .token;
        (id, self.server.client_config_as(&token))
    }

    /// Creates a Movies Library holding Alien, scans it, and waits until Alien is Matched.
    async fn library_with_alien(&self) -> tempfile::TempDir {
        let media = tempfile::tempdir().unwrap();
        let folder = media.path().canonicalize().unwrap();
        let file = folder.join("Alien (1979)/Alien (1979).mkv");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, b"video").unwrap();
        let config = self.server.client_config();
        let libraries = LibraryServiceClient::new(HttpClient::plaintext(), config.clone());
        let library = libraries
            .create_library(CreateLibraryRequest {
                name: "Movies".to_owned(),
                kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
                folders: vec![folder.to_str().unwrap().to_owned()],
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .library
            .into_option()
            .unwrap()
            .id;
        libraries
            .scan_library(ScanLibraryRequest {
                id: library,
                ..Default::default()
            })
            .await
            .unwrap();
        let browse = BrowseServiceClient::new(HttpClient::plaintext(), config);
        tokio::time::timeout(WAIT, until_a_movie_is_matched(&browse, library))
            .await
            .unwrap();
        media
    }
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn until_a_movie_is_matched(browse: &BrowseServiceClient<HttpClient>, library: i64) {
    let listing = ListMoviesRequest {
        library_id: library,
        ..Default::default()
    };
    while browse
        .list_movies(listing.clone())
        .await
        .unwrap()
        .into_owned()
        .movies
        .is_empty()
    {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn requests(config: ClientConfig) -> RequestServiceClient<HttpClient> {
    RequestServiceClient::new(HttpClient::plaintext(), config)
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn ask(client: &RequestServiceClient<HttpClient>, tmdb_id: i64) -> Request {
    client
        .create_request(CreateRequestRequest {
            movie_tmdb_id: tmdb_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap()
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn list(client: &RequestServiceClient<HttpClient>) -> Vec<Request> {
    client
        .list_requests(ListRequestsRequest::default())
        .await
        .unwrap()
        .into_owned()
        .requests
}

fn requester_ids(request: &Request) -> Vec<i64> {
    request
        .requesters
        .iter()
        .map(|requester| requester.user_id)
        .collect()
}

#[tokio::test]
async fn two_users_asking_for_the_same_movie_share_one_request_with_both_as_requesters() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &MEMBER).await;
    let (grace, as_grace) = fixture.user("Grace", &MEMBER).await;

    let first = ask(&requests(as_ada), HEAT).await;
    let second = ask(&requests(as_grace.clone()), HEAT).await;
    let again = ask(&requests(as_grace.clone()), HEAT).await;

    assert_eq!(second.id, first.id);
    assert_eq!(requester_ids(&second), [ada, grace]);
    assert_eq!(again, second, "asking twice changes nothing");
    let movie = second.movie.as_option().map(|movie| movie.title.as_str());
    assert_eq!(movie, Some("Heat"), "{second:?}");
    assert_eq!(second.requesters[1].name, "Grace");
    assert_eq!(list(&requests(as_grace)).await, [second]);
}

#[tokio::test]
async fn a_user_who_may_auto_approve_movies_opens_an_approved_request_and_one_who_may_not_a_pending_one()
 {
    let fixture = Fixture::start().await;
    let (_, auto) = fixture
        .user(
            "Ada",
            &[Permission::RequestMovie, Permission::AutoApproveMovie],
        )
        .await;
    let (_, covered) = fixture
        .user("Linus", &[Permission::Request, Permission::AutoApprove])
        .await;
    let (_, member) = fixture.user("Grace", &MEMBER).await;

    let approved = ask(&requests(auto), HEAT).await;
    let also_approved = ask(&requests(covered), ALIEN).await;
    let pending = ask(&requests(member), DUNE).await;

    assert_eq!(approved.state, RequestState::Approved);
    assert_eq!(also_approved.state, RequestState::Approved);
    assert_eq!(pending.state, RequestState::Pending);
}

#[tokio::test]
async fn a_user_without_request_movie_is_refused_naming_the_permission() {
    let fixture = Fixture::start().await;
    let (_, as_viewer) = fixture.user("Ada", &[Permission::ViewRequests]).await;

    let refused = requests(as_viewer)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: HEAT,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_REQUEST_MOVIE"));
}

#[tokio::test]
async fn requesting_a_movie_matched_in_a_library_is_refused() {
    let fixture = Fixture::start().await;
    let _media = fixture.library_with_alien().await;
    let (_, as_member) = fixture.user("Grace", &MEMBER).await;

    let refused = requests(as_member)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: ALIEN,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::FailedPrecondition,
        RefusalReason::REFUSAL_REASON_ALREADY_IN_LIBRARY,
    );
    assert_eq!(refusal.field, "movie_tmdb_id");
}

#[tokio::test]
async fn requesting_a_movie_the_metadata_source_doesnt_have_is_refused() {
    let fixture = Fixture::start().await;
    let (_, as_member) = fixture.user("Grace", &MEMBER).await;

    let refused = requests(as_member)
        .create_request(CreateRequestRequest {
            movie_tmdb_id: 1,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    let title = refusal.params.get("titleId").map(String::as_str);
    assert_eq!(title, Some("tmdb-1"));
}

#[tokio::test]
async fn without_view_requests_a_user_lists_and_gets_only_their_own_requests() {
    let fixture = Fixture::start().await;
    let (_, as_ada) = fixture.user("Ada", &MEMBER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let adas = ask(&requests(as_ada.clone()), HEAT).await;
    ask(&requests(as_ada.clone()), DUNE).await;
    let grace = requests(as_grace);
    let shared = ask(&grace, DUNE).await;

    let listed = list(&grace).await;
    let got_shared = grace
        .get_request(GetRequestRequest {
            id: shared.id,
            ..Default::default()
        })
        .await;
    let got_adas = grace
        .get_request(GetRequestRequest {
            id: adas.id,
            ..Default::default()
        })
        .await;

    assert_eq!(listed, std::slice::from_ref(&shared));
    let got_shared = got_shared.map(|response| response.into_owned().request.into_option());
    assert_eq!(got_shared.ok().flatten(), Some(shared.clone()));
    assert_refused(
        got_adas,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_REQUEST_NOT_FOUND,
    );
    assert_eq!(
        list(&requests(as_ada)).await,
        [shared, adas],
        "newest first"
    );
}

#[tokio::test]
async fn without_view_requests_the_events_stream_shows_only_the_users_own_requests() {
    let fixture = Fixture::start().await;
    let (ada, as_ada) = fixture.user("Ada", &MEMBER).await;
    let (_, as_grace) = fixture.user("Grace", &[Permission::RequestMovie]).await;
    let only_requests = SubscribeRequest {
        types: vec![
            EventType::EVENT_TYPE_REQUEST_CREATED.into(),
            EventType::EVENT_TYPE_REQUESTER_ADDED.into(),
        ],
        ..Default::default()
    };
    let mut graces_events = EventServiceClient::new(HttpClient::plaintext(), as_grace.clone())
        .subscribe(only_requests)
        .await
        .unwrap();

    ask(&requests(as_ada.clone()), HEAT).await;
    let shared = ask(&requests(as_ada), DUNE).await;
    ask(&requests(as_grace), DUNE).await;
    let next = tokio::time::timeout(WAIT, graces_events.message::<SubscribeResponse>())
        .await
        .expect("timed out waiting for an event")
        .unwrap()
        .expect("the stream ended")
        .to_owned_message()
        .message;

    let Some(subscribe_response::Message::Event(event)) = next else {
        panic!("not an event: {next:?}");
    };
    let Some(event::Event::RequesterAdded(added)) = event.event else {
        panic!("Ada's Requests reached Grace first: {event:?}");
    };
    assert_eq!(added.request.as_option().map(|r| r.id), Some(shared.id));
    assert_eq!(
        added.request.as_option().map(requester_ids),
        Some(vec![ada, added.user_id])
    );
}
