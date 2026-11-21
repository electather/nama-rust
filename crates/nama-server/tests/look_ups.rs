//! Looking a Movie or Show up by its id through the generated `RequestService` client, over a
//! real socket, with the TMDB Plugin enabled against a fake TMDB.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{
    ErrorCode,
    client::{ClientConfig, HttpClient},
};
use nama_proto::nama::v1::{Permission, RefusalReason, RequestServiceClient, Role};
use nama_server::{config::Config, plugins::Registry};
use refused::assert_refused;
use server::{
    TestServer,
    requests::{Requester, title_id, tmdb_id},
    tmdb::{ALIEN, DARK, DARK_TVDB},
};

/// What a new User gets: `REQUEST` and `VIEW_REQUESTS`.
const NEW_USER: [Permission; 2] = [Permission::Request, Permission::ViewRequests];

/// A Server with TMDB on, and an Admin.
struct Fixture {
    server: TestServer,
}

impl Fixture {
    async fn start() -> Self {
        let builder = TestServer::builder().config(Config::default());
        let server = builder.plugins(Registry::new()).tmdb().start().await;
        Self { server }
    }
}

fn requests(config: ClientConfig) -> RequestServiceClient<HttpClient> {
    RequestServiceClient::new(HttpClient::plaintext(), config)
}

#[tokio::test]
async fn looking_up_a_movie_no_metadata_source_has_is_not_found() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    for id in [tmdb_id(1), title_id("anidb", "1")] {
        let refused = as_grace.try_look_up_movie(id.clone()).await;

        let refusal = assert_refused(
            refused,
            ErrorCode::NotFound,
            RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
        );
        let title_id = format!("{}-{}", id.namespace, id.value);
        assert_eq!(refusal.params.get("titleId"), Some(&title_id));
    }
}

#[tokio::test]
async fn looking_up_a_movie_by_a_malformed_id_is_refused() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    let refused = as_grace.try_look_up_movie(title_id("Ani-DB", "1")).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TITLE_ID_MALFORMED,
    );
    assert_eq!(refusal.field, "movie_id");
}

#[tokio::test]
async fn looking_up_a_show_no_metadata_source_has_is_not_found() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &NEW_USER)
        .await
        .client_config();

    let refused = requests(as_grace).try_look_up_show(tmdb_id(1)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "show_id");
}

#[tokio::test]
async fn a_user_without_request_tv_is_refused_looking_up_a_show() {
    let fixture = Fixture::start().await;
    let as_grace = fixture
        .server
        .user("Grace", &[Permission::RequestMovie])
        .await
        .client_config();

    let refused = requests(as_grace).try_look_up_show(tmdb_id(DARK)).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::PermissionDenied,
        RefusalReason::REFUSAL_REASON_PERMISSION_REQUIRED,
    );
    let permission = refusal.params.get("permission").map(String::as_str);
    assert_eq!(permission, Some("PERMISSION_REQUEST_TV"));
}

#[tokio::test]
async fn looking_up_a_show_by_a_malformed_id_is_refused() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    for id in [title_id("Ani-DB", "1"), title_id("tvdb", "")] {
        let refused = as_grace.try_look_up_show(id).await;

        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_TITLE_ID_MALFORMED,
        );
        assert_eq!(refusal.field, "show_id");
    }
}

#[tokio::test]
async fn looking_up_a_show_in_a_namespace_no_metadata_source_knows_is_not_found() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    let refused = as_grace.try_look_up_show(title_id("anidb", "1")).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_TITLE_NOT_FOUND,
    );
    assert_eq!(refusal.field, "show_id");
}

#[tokio::test]
async fn looking_up_a_show_by_its_tvdb_id_finds_it_under_its_preferred_id() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    let by_tvdb = title_id("tvdb", &DARK_TVDB.to_string());
    let found = as_grace.try_look_up_show(by_tvdb).await.unwrap();

    assert_eq!(found.id.into_option(), Some(tmdb_id(DARK)));
    assert_eq!(found.title, "Dark");
}

#[tokio::test]
async fn a_looked_up_movie_and_show_have_their_score_and_original_language() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    let alien = as_grace.try_look_up_movie(tmdb_id(ALIEN)).await.unwrap();
    let dark = as_grace.try_look_up_show(tmdb_id(DARK)).await.unwrap();

    let alien_language = alien.original_language.as_deref();
    assert_eq!((alien.score, alien_language), (Some(8.2), Some("en")));
    let dark_language = dark.original_language.as_deref();
    assert_eq!((dark.score, dark_language), (Some(8.4), Some("de")));
}

#[tokio::test]
async fn a_looked_up_movie_and_show_have_their_cast_and_crew() {
    let fixture = Fixture::start().await;
    let grace = fixture.server.user("Grace", &NEW_USER).await;
    let as_grace = fixture.server.requests(&grace);

    let alien = as_grace.try_look_up_movie(tmdb_id(ALIEN)).await.unwrap();
    let dark = as_grace.try_look_up_show(tmdb_id(DARK)).await.unwrap();

    let names = |roles: &[Role]| {
        roles
            .iter()
            .map(|role| role.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&alien.cast), ["Tom Skerritt", "Sigourney Weaver"]);
    assert_eq!(names(&alien.crew), ["Ridley Scott", "Dan O'Bannon"]);
    assert!(alien.cast[1].headshot_url.is_some());
    let dark_crew = ["Baran bo Odar", "Jantje Friese", "Baran bo Odar"];
    assert_eq!(names(&dark.crew), dark_crew);
}
