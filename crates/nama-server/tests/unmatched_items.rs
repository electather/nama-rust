//! Listing a Library's Unmatched Items through the generated `MatchService` client, over a real
//! socket: which Library, and its pages.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    LibraryKind, ListUnmatchedItemsRequest, MatchServiceClient, PageRequest, RefusalReason,
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
