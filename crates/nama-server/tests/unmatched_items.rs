//! Listing a Library's Unmatched Items through the generated `MatchService` and `BrowseService`
//! clients, over a real socket: which Library, its pages, and what a User without Permissions sees
//! and plays.

#[expect(dead_code, reason = "this binary uses only some of the helpers")]
#[path = "support/http.rs"]
mod http;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use connectrpc::{ErrorCode, client::HttpClient};
use http::fetch;
use nama_proto::nama::v1::{
    BrowseServiceClient, GetUnmatchedVersionRequest, LibraryKind, ListUnmatchedItemsRequest,
    ListUnmatchedRequest, MatchServiceClient, PageRequest, RefusalReason, TitleKind,
    UnmatchedItemFilter,
};
use refused::assert_refused;
use server::TestServer;

/// `ListUnmatchedItems`' `page` of the Library `library`'s items.
fn in_library(library: i64, page: PageRequest) -> ListUnmatchedItemsRequest {
    let filter = UnmatchedItemFilter {
        library_id: library,
        ..Default::default()
    };
    ListUnmatchedItemsRequest {
        filter: filter.into(),
        page: page.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn listing_the_unmatched_items_of_a_library_that_does_not_exist_is_not_found() {
    let server = TestServer::builder().start().await;
    let films = server
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[])
        .await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());

    let refused = matches
        .list_unmatched_items(in_library(films.id + 1, PageRequest::default()))
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_page_token_reused_for_another_library_is_refused() {
    let server = TestServer::builder().start().await;
    let movies = LibraryKind::LIBRARY_KIND_MOVIES;
    // No title source, so both stay Unmatched.
    let films = (server.matched_library(movies, &["Alien (1979).mkv", "Heat (1995).mkv"])).await;
    let other = server.create_library(movies, &[]).await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let one = PageRequest {
        size: 1,
        ..Default::default()
    };
    let first = matches
        .list_unmatched_items(in_library(films.id, one))
        .await;
    let token = first
        .unwrap()
        .into_owned()
        .page
        .into_option()
        .unwrap()
        .next_token;

    let carried_over = PageRequest {
        token,
        ..Default::default()
    };
    let refused = matches
        .list_unmatched_items(in_library(other.id, carried_over))
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
    );
    assert_eq!(refusal.field, "page.token");
}

#[tokio::test]
async fn files_whose_names_do_not_parse_come_on_the_first_page_only() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv", "home video.mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let one = PageRequest {
        size: 1,
        ..Default::default()
    };
    let first = matches
        .list_unmatched_items(in_library(films.id, one))
        .await;
    let first = first.unwrap().into_owned();

    let next = PageRequest {
        token: first.page.next_token.clone(),
        ..Default::default()
    };
    let second = matches
        .list_unmatched_items(in_library(films.id, next))
        .await;

    assert_eq!(first.unparsed_paths, [films.path("home video.mkv")]);
    assert_eq!(
        second.unwrap().into_owned().unparsed_paths,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn a_user_without_permissions_lists_unmatched_items_by_parsed_name_newest_first() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv", "home video.mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());

    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    let parsed: Vec<_> = (listed.items.iter())
        .map(|item| (item.kind.as_known(), item.title.as_str(), item.year))
        .collect();
    let movie = Some(TitleKind::TITLE_KIND_MOVIE);
    assert_eq!(
        parsed,
        [(movie, "Heat", Some(1995)), (movie, "Alien", Some(1979))]
    );
    assert!(listed.items.iter().all(|item| item.version_id > 0));
    assert_eq!(listed.page.total_size, Some(2));
}

#[tokio::test]
async fn listing_the_unmatched_items_of_no_library_as_a_user_is_not_found() {
    let server = TestServer::builder().start().await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());

    let refused = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: 404,
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

#[tokio::test]
async fn a_users_page_token_holds_no_path_and_lists_the_next_page() {
    let server = TestServer::builder().start().await;
    let names = ["Alien (1979).mkv", "Heat (1995).mkv"];
    let films = (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &names)).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let page = |size, token| ListUnmatchedRequest {
        library_id: films.id,
        page: PageRequest {
            size,
            token,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };

    let first = browse.list_unmatched(page(1, String::new())).await;
    let token = first
        .unwrap()
        .into_owned()
        .page
        .into_option()
        .unwrap()
        .next_token;
    let second = browse.list_unmatched(page(0, token.clone())).await;

    let cursor = String::from_utf8(URL_SAFE_NO_PAD.decode(&token).unwrap()).unwrap();
    assert!(!cursor.contains("Heat"), "{cursor}");
    let second = second.unwrap().into_owned();
    let titles: Vec<_> = second
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["Alien"]);
}

#[tokio::test]
async fn a_user_gets_an_unmatched_items_version_by_its_id_and_plays_its_file() {
    let server = TestServer::builder().start().await;
    let films =
        (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien (1979).mkv"])).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let admins = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = admins
        .list_unmatched_items(in_library(films.id, PageRequest::default()))
        .await
        .unwrap()
        .into_owned();
    let version_id = listed.items[0].version_id;

    let got = browse
        .get_unmatched_version(GetUnmatchedVersionRequest {
            library_id: films.id,
            version_id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let played = fetch(
        &server,
        &format!("/media/versions/{version_id}"),
        Some(&ripley.token),
    )
    .await;

    assert_eq!(got.item.title, "Alien");
    assert_eq!(got.item.version_id, version_id);
    assert_eq!(got.version.id, version_id);
    assert_eq!(got.version.file_name, "Alien (1979).mkv");
    assert_eq!(played.status, 200);
}

#[tokio::test]
async fn getting_a_version_of_another_library_is_not_found() {
    let server = TestServer::builder().start().await;
    let movies = LibraryKind::LIBRARY_KIND_MOVIES;
    let films = (server.matched_library(movies, &["Alien (1979).mkv"])).await;
    let other = server.create_library(movies, &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    let refused = browse
        .get_unmatched_version(GetUnmatchedVersionRequest {
            library_id: other.id,
            version_id: listed.items[0].version_id,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_VERSION_NOT_FOUND,
    );
    assert_eq!(refusal.field, "version_id");
}

#[tokio::test]
async fn a_file_whose_name_does_not_parse_is_not_served() {
    let server = TestServer::builder().start().await;
    let films =
        (server.matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &["home video.mkv"])).await;
    let ripley = server.user("Ripley", &[]).await;
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), ripley.client_config());
    let listed = browse
        .list_unmatched(ListUnmatchedRequest {
            library_id: films.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    // The scan stores it without an Item, under the only id the Library has given out.
    let played = fetch(&server, "/media/versions/1", Some(&ripley.token)).await;

    assert_eq!(listed.items.len(), 0);
    assert_eq!(played.status, 404);
}
