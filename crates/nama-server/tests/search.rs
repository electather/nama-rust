//! Searching every Library a page at a time (ADR 0055), over a real socket with the TMDB Plugin
//! enabled against a fake TMDB.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::{ALIEN_1080P, Fixture};
use connectrpc::{ConnectError, ErrorCode};
use nama_proto::nama::v1::{
    LibraryKind, PageRequest, RefusalReason, SearchLibrariesRequest, SearchLibrariesResponse,
};
use refused::assert_refused;

impl Fixture {
    /// The page of at most `size` titles a search for `query` finds after the one that returned
    /// `token`, or the first for an empty one.
    async fn search_page(
        &self,
        query: &str,
        size: i32,
        token: &str,
    ) -> Result<SearchLibrariesResponse, ConnectError> {
        let page = PageRequest {
            size,
            token: token.to_owned(),
            ..Default::default()
        };
        let asked = SearchLibrariesRequest {
            query: query.to_owned(),
            page: page.into(),
            ..Default::default()
        };
        Ok(self.browse.search_libraries(asked).await?.into_owned())
    }
}

#[tokio::test]
async fn a_search_lists_a_page_and_refuses_its_token_for_another_query() {
    let fx = Fixture::alien().await;
    let other = (fx.server)
        .create_library(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN_1080P])
        .await;
    fx.server.scan_and_match(&other).await;

    let first = fx.search_page("alien", 1, "").await.unwrap();
    let second = fx.search_page("alien", 1, &first.page.next_token).await;
    let other_query = fx.search_page("alie", 1, &first.page.next_token).await;

    assert_eq!(first.movies.len(), 1);
    assert_eq!(
        first.page.total_size,
        Some(2),
        "Alien, once in each Library"
    );
    let second = second.unwrap();
    assert_eq!(second.page.next_token, "", "the last page");
    let libraries = [&first, &second].map(|page| page.movies[0].library_id);
    assert_ne!(libraries[0], libraries[1]);
    let refusal = assert_refused(
        other_query,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_PAGE_TOKEN_INVALID,
    );
    assert_eq!(refusal.field, "page.token");
}
