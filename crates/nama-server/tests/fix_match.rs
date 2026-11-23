//! Fixing a Matched Movie's or Show's Match by hand, as its page does, through the generated
//! `MatchService` client, over a real socket, with the TMDB Plugin enabled against a fake TMDB.

#[path = "support/browsing.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod browsing;
#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use browsing::movies_in;
use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, ClearMatchRequest, ExternalId, LibraryKind, ListEpisodesRequest,
    ListUnmatchedItemsRequest, MatchServiceClient, MatchedTitle, RefusalReason,
    SearchTitlesRequest, SetMatchRequest, TitleKind, UnmatchedItemFilter, UnmatchedReason,
};
use refused::assert_refused;
use server::{Library, TestServer};

/// Two files of Alien named differently, so each name is pinned on its own.
const ALIEN: &str = "Alien (1979)/Alien (1979).mkv";
const ALIEN_CUT: &str = "Alien (1979)/Alien {tmdb-348}.mkv";

/// Two Episodes of Dark in folders named differently.
const DARK_E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
const DARK_E02: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E02.mkv";

struct Fixture {
    server: TestServer,
    media: Library,
    matches: MatchServiceClient<HttpClient>,
    browse: BrowseServiceClient<HttpClient>,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// A Server with TMDB and a Library of `kind` holding `files`, scanned and Matched.
    async fn start(kind: LibraryKind, files: &[&str]) -> Self {
        let server = TestServer::builder().tmdb().start().await;
        let media = server.create_library(kind, &[]).await;
        for file in files {
            media.write(file, b"video");
        }
        server.scan_and_match(&media).await;
        Self {
            matches: MatchServiceClient::new(HttpClient::plaintext(), server.client_config()),
            browse: BrowseServiceClient::new(HttpClient::plaintext(), server.client_config()),
            server,
            media,
        }
    }

    fn title(&self, tmdb: i64) -> MatchedTitle {
        MatchedTitle {
            library_id: self.media.id,
            id: tmdb_id(tmdb).into(),
            ..Default::default()
        }
    }

    /// Each Movie of the Library, by TMDB id, with how many Versions it has.
    async fn movies(&self) -> Vec<(String, usize)> {
        let movies = movies_in(&self.browse, self.media.id).await.unwrap();
        (movies.into_iter())
            .map(|movie| (movie.id.value.clone(), movie.versions.len()))
            .collect()
    }

    /// The TMDB ids of the Episodes of Season 1 of the Show `show`.
    async fn season_1(&self, show: i64) -> Vec<String> {
        let listed = (self.browse)
            .list_episodes(ListEpisodesRequest {
                library_id: self.media.id,
                show_id: tmdb_id(show).into(),
                season_number: 1,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        (listed.episodes.into_iter())
            .map(|episode| episode.id.value.clone())
            .collect()
    }
}

fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn fixing_a_movies_match_moves_every_version_and_holds_once_rescanned() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN, ALIEN_CUT]).await;
    let before = fx.movies().await;

    let fixed = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: fx.title(348).into(),
            title_id: tmdb_id(949).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let after = fx.movies().await;
    fx.media.write(ALIEN, b"a better copy of the video");
    fx.server.scan_and_match(&fx.media).await;

    assert_eq!(before, [("348".to_owned(), 2)]);
    assert_eq!((fixed.matched, fixed.unmatched), (2, 0));
    assert_eq!(after, [("949".to_owned(), 2)]);
    assert_eq!(fx.movies().await, after);
}

#[tokio::test]
async fn fixing_a_shows_match_moves_every_episode() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_E01, DARK_E02]).await;
    let before = fx.season_1(70_523).await;

    let fixed = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: fx.title(70_523).into(),
            title_id: tmdb_id(2316).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert_eq!(before.len(), 2);
    assert_eq!(fixed.matched, 2);
    assert_eq!(fx.season_1(2316).await, ["2316001", "2316002"]);
}

#[tokio::test]
async fn unmatching_a_movie_lists_every_version_as_cleared_by_hand() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN, ALIEN_CUT]).await;

    let cleared = (fx.matches)
        .clear_match(ClearMatchRequest {
            matched_title: fx.title(348).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let listed = (fx.matches)
        .list_unmatched_items(ListUnmatchedItemsRequest {
            filter: UnmatchedItemFilter {
                library_id: fx.media.id,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert_eq!(cleared.unmatched, 2);
    let reasons: Vec<_> = (listed.items.iter())
        .map(|item| item.reason.as_known())
        .collect();
    let cleared_by_hand = Some(UnmatchedReason::UNMATCHED_REASON_CLEARED_BY_HAND);
    assert_eq!(reasons, [cleared_by_hand, cleared_by_hand]);
    assert_eq!(fx.movies().await, []);
}

#[tokio::test]
async fn fixing_the_match_of_a_title_the_library_lacks_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN]).await;

    let refused = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: fx.title(949).into(),
            title_id: tmdb_id(348).into(),
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_NOTHING_MATCHED,
    );
}

#[tokio::test]
async fn searching_a_librarys_title_sources_gives_each_title_its_poster() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES, &[]).await;
    let search = |library_id| SearchTitlesRequest {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        title: "Alien".to_owned(),
        year: Some(1979),
        library_id: Some(library_id),
        ..Default::default()
    };

    let found = (fx.matches.search_titles(search(fx.media.id)).await)
        .unwrap()
        .into_owned();
    let no_library = fx.matches.search_titles(search(fx.media.id + 1)).await;

    let poster = found.candidates[0]
        .poster_url
        .as_deref()
        .unwrap_or_default();
    assert!(poster.starts_with("/media/artwork/"), "{poster}");
    assert_refused(
        no_library,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}
