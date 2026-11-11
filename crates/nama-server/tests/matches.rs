//! Fixing Matches by hand through the generated `MatchService` client, over a real socket, with
//! the TMDB Plugin enabled against a fake TMDB.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{collections::HashMap, fs, path::Path};

use buffa::EnumValue;
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, ClearMatchRequest, CreateRequestRequest, CreateSeasonRequestsRequest,
    ExternalId, GetRequestRequest, LibraryKind, ListEpisodesRequest, ListMoviesRequest,
    ListUnmatchedItemsRequest, ListUnmatchedItemsResponse, MatchServiceClient, RefusalReason,
    RequestServiceClient, RequestState, SearchMetadataRequest, SetMatchRequest, SetMatchResponse,
    TitleCandidate, TitleKind, UnmatchedItem, UnmatchedReason, external_id,
};
use refused::assert_refused;
use server::{Builder, Library, TestServer};

/// TMDB finds nothing for this file, so it is Unmatched until Matched by hand.
const HOME_MOVIES: &str = "Home Movies (2019)/Home Movies (2019).mkv";

/// What a file holds when the test first writes it.
const VIDEO: &[u8] = b"video";

/// A Server with one Library, usually the TMDB Plugin enabled against a fake TMDB, and clients
/// for its `MatchService` and `BrowseService`.
struct Fixture {
    library: i64,
    folder: std::path::PathBuf,
    matches: MatchServiceClient<HttpClient>,
    browse: BrowseServiceClient<HttpClient>,
    server: TestServer,
    media: Library,
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
impl Fixture {
    /// Starts a Server with a Library of `kind`.
    async fn start(kind: LibraryKind) -> Self {
        Self::start_with(kind, TestServer::builder().tmdb()).await
    }

    /// Starts a Server with a Library of `kind` and no metadata Provider, so nothing is Matched.
    async fn start_without_metadata(kind: LibraryKind) -> Self {
        Self::start_with(kind, TestServer::builder()).await
    }

    /// Starts the Server `builder` makes, with a Library of `kind`.
    async fn start_with(kind: LibraryKind, builder: Builder) -> Self {
        let server = builder.start().await;
        let media = server.create_library(kind, &[]).await;
        Self {
            library: media.id,
            folder: media.folder().to_owned(),
            matches: MatchServiceClient::new(HttpClient::plaintext(), server.client_config()),
            browse: BrowseServiceClient::new(HttpClient::plaintext(), server.client_config()),
            server,
            media,
        }
    }

    fn write(&self, relative: &str, contents: &[u8]) {
        self.media.write(relative, contents);
    }

    /// Scans the Library until Matching has finished, and returns the first Item the scan stored
    /// for each file, by its path in the Library.
    async fn scan_and_match(&self) -> HashMap<String, i64> {
        self.server.scan_and_match(&self.media).await
    }

    /// The id of the Movie `ListMovies` lists the file at `relative` under, or `None` while it is
    /// Unmatched.
    async fn movie_of(&self, relative: &str) -> Option<ExternalId> {
        let file_name = Path::new(relative).file_name().unwrap().to_str().unwrap();
        let movies = self
            .browse
            .list_movies(ListMoviesRequest {
                library_id: self.library,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .movies;
        let mut holding = movies
            .into_iter()
            .filter(|movie| (movie.versions.iter()).any(|version| version.file_name == file_name));
        holding.next().and_then(|movie| movie.id.into_option())
    }

    /// The ids of the Episodes in Season 1 of the Show with the TMDB id `show`, as
    /// `ListEpisodes` lists them.
    async fn season_1(&self, show: i64) -> Vec<Option<ExternalId>> {
        let episodes = self
            .browse
            .list_episodes(ListEpisodesRequest {
                library_id: self.library,
                show_id: tmdb_id(show).into(),
                season_number: 1,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
            .episodes;
        episodes
            .into_iter()
            .map(|episode| episode.id.into_option())
            .collect()
    }

    async fn set_match(&self, item: i64, tmdb: i64) -> Result<SetMatchResponse, ConnectError> {
        Ok(self
            .matches
            .set_match(SetMatchRequest {
                item_id: item,
                title_id: tmdb_id(tmdb).into(),
                ..Default::default()
            })
            .await?
            .into_owned())
    }

    /// The Library's items that need a Match, as `ListUnmatchedItems` lists them.
    async fn listed(&self) -> Vec<UnmatchedItem> {
        self.needs_a_match().await.items
    }

    /// The paths of the Library's files that hold no items, as `ListUnmatchedItems` lists them.
    async fn unparsed(&self) -> Vec<String> {
        self.needs_a_match().await.unparsed_paths
    }

    /// The Needs a Match list, as `ListUnmatchedItems` gives it.
    async fn needs_a_match(&self) -> ListUnmatchedItemsResponse {
        self.matches
            .list_unmatched_items(ListUnmatchedItemsRequest {
                library_id: self.library,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned()
    }

    /// The absolute path of the file at `relative` in the Library's folder.
    fn path(&self, relative: &str) -> String {
        self.media.path(relative)
    }
}

fn tmdb_id(id: i64) -> ExternalId {
    ExternalId {
        id: Some(external_id::Id::Tmdb(id)),
        ..Default::default()
    }
}

#[tokio::test]
async fn an_unmatched_file_matched_by_hand_stays_matched_when_it_changes_and_is_rescanned() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];
    let before = fx.movie_of(HOME_MOVIES).await;

    let response = fx.set_match(item, 949).await.unwrap();
    fx.write(HOME_MOVIES, b"a better copy of the video");
    let rescanned = fx.scan_and_match().await;
    let after = fx.movie_of(HOME_MOVIES).await;

    assert_eq!(before, None);
    assert_eq!((response.matched, response.unmatched), (1, 0));
    assert_ne!(rescanned.get(HOME_MOVIES), Some(&item));
    assert_eq!(after, Some(tmdb_id(949)));
}

#[tokio::test]
async fn a_cleared_match_is_unmatched() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    fx.write(alien, VIDEO);
    let item = fx.scan_and_match().await[alien];
    let before = fx.movie_of(alien).await;

    let response = fx
        .matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    assert_eq!(before, Some(tmdb_id(348)));
    assert_eq!(response.unmatched, 1);
    assert_eq!(fx.movie_of(alien).await, None);
}

#[tokio::test]
async fn search_metadata_passes_the_search_to_the_metadata_provider() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let response = fx
        .matches
        .search_metadata(SearchMetadataRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            title: "Alien".to_owned(),
            year: Some(1979),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();

    let best = &response.candidates[0];
    assert_eq!(
        (best.id.id.clone(), best.title.as_str(), best.year),
        (Some(external_id::Id::Tmdb(348)), "Alien", Some(1979))
    );
}

#[tokio::test]
async fn search_metadata_without_a_kind_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let refused = fx
        .matches
        .search_metadata(SearchMetadataRequest {
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_ENUM_VALUE,
    );
    assert_eq!(refusal.field, "kind");
}

#[tokio::test]
async fn search_metadata_without_a_metadata_provider_is_unavailable() {
    let fx = Fixture::start_without_metadata(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let refused = fx
        .matches
        .search_metadata(SearchMetadataRequest {
            kind: TitleKind::TITLE_KIND_MOVIE.into(),
            title: "Alien".to_owned(),
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::Unavailable,
        RefusalReason::REFUSAL_REASON_NO_METADATA_PROVIDER,
    );
}

#[tokio::test]
async fn matching_a_file_by_hand_to_a_requested_movie_makes_its_request_available() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];
    let requests = RequestServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let heat = requests
        .create_request(CreateRequestRequest {
            movie_tmdb_id: 949,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap();

    fx.set_match(item, 949).await.unwrap();
    let after = requests
        .get_request(GetRequestRequest {
            id: heat.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap();

    assert_eq!(after.state, RequestState::Available);
}

#[tokio::test]
async fn matching_one_of_two_aired_episodes_by_hand_makes_its_processing_season_request_partially_available()
 {
    let fx = Fixture::start_with(
        LibraryKind::LIBRARY_KIND_SHOWS,
        TestServer::builder().tmdb().shows_fulfiller(&[]),
    )
    .await;
    let pilot = "The Office/Season 01/The Office - S01E01.mkv";
    fx.write(pilot, VIDEO);
    // Both the US and the UK Office fit, so the scan leaves it Unmatched.
    let item = fx.scan_and_match().await[pilot];
    let requests = RequestServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let season_1 = requests
        .create_season_requests(CreateSeasonRequestsRequest {
            show_tmdb_id: 2316,
            season_numbers: vec![1],
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .requests
        .remove(0);

    fx.set_match(item, 2316).await.unwrap();
    let after = requests
        .get_request(GetRequestRequest {
            id: season_1.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .request
        .into_option()
        .unwrap();

    assert_eq!(after.state, RequestState::PartiallyAvailable);
}

#[tokio::test]
async fn set_match_for_an_item_that_does_not_exist_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let refused = fx.set_match(42, 949).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_ITEM_NOT_FOUND,
    );
    assert_eq!(refusal.field, "item_id");
}

#[tokio::test]
async fn set_match_to_a_title_tmdb_does_not_have_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];

    let refused = fx.set_match(item, 1).await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_UNKNOWN_TITLE,
    );
    assert_eq!(refusal.field, "title_id");
    let title_id = refusal.params.get("titleId").map(String::as_str);
    assert_eq!(title_id, Some("tmdb-1"));
}

#[tokio::test]
async fn set_match_without_a_title_is_invalid() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];

    let refused = fx
        .matches
        .set_match(SetMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await;

    let refusal = assert_refused(
        refused,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
    );
    assert_eq!(refusal.field, "title_id");
}

#[tokio::test]
async fn re_matching_a_show_by_hand_re_matches_its_episodes() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
    let pilot = "The Office/Season 01/The Office - S01E01.mkv";
    let second = "The Office/Season 01/The Office - S01E02.mkv";
    fx.write(pilot, VIDEO);
    fx.write(second, VIDEO);
    // Both the US and the UK Office fit, so the scan leaves them Unmatched.
    let item = fx.scan_and_match().await[pilot];
    let before = fx.listed().await;

    let to_american = fx.set_match(item, 2316).await.unwrap();
    let us = fx.season_1(2316).await;
    let to_british = fx.set_match(item, 2996).await.unwrap();
    let uk = fx.season_1(2996).await;

    assert_eq!(before.len(), 2);
    assert_eq!((to_american.matched, to_british.matched), (2, 2));
    let tmdb = |id| Some(tmdb_id(id));
    assert_eq!(us, [tmdb(2_316_001), tmdb(2_316_002)]);
    assert_eq!(uk, [tmdb(2_996_001), tmdb(2_996_002)]);
}

/// The Movie `title` from `year` held by the file at `path`, as `ListUnmatchedItems` lists it
/// when no title fits it.
fn unmatched_movie(item: i64, path: String, title: &str, year: i32) -> UnmatchedItem {
    UnmatchedItem {
        item_id: item,
        path,
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        title: title.to_owned(),
        year: Some(year),
        reason: UnmatchedReason::UNMATCHED_REASON_NO_TITLE_FITS.into(),
        ..Default::default()
    }
}

/// The Show "The Office" first aired in `year`, as `ListUnmatchedItems` lists a tied title.
fn the_office(tmdb: i64, year: i32) -> TitleCandidate {
    TitleCandidate {
        id: tmdb_id(tmdb).into(),
        title: "The Office".to_owned(),
        original_title: Some("The Office".to_owned()),
        year: Some(year),
        ..Default::default()
    }
}

#[tokio::test]
async fn an_ambiguous_item_is_listed_with_the_titles_that_tied() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_SHOWS).await;
    fx.write("The Office/Season 01/The Office - S01E01.mkv", VIDEO);
    fx.scan_and_match().await;

    let listed = fx.listed().await;

    assert_eq!(
        listed
            .iter()
            .map(|item| (item.reason, item.tied_titles.clone()))
            .collect::<Vec<_>>(),
        [(
            EnumValue::from(UnmatchedReason::UNMATCHED_REASON_AMBIGUOUS),
            vec![the_office(2316, 2005), the_office(2996, 2001)]
        )]
    );
}

#[tokio::test]
async fn an_item_whose_id_hint_names_no_title_is_listed_as_such() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write("Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv", VIDEO);
    fx.scan_and_match().await;

    let listed = fx.listed().await;

    assert_eq!(
        listed.iter().map(|item| item.reason).collect::<Vec<_>>(),
        [EnumValue::from(
            UnmatchedReason::UNMATCHED_REASON_UNKNOWN_ID_HINT
        )]
    );
}

#[tokio::test]
async fn an_item_not_matched_yet_is_listed_without_a_reason() {
    let fx = Fixture::start_without_metadata(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    fx.scan_and_match().await;

    let listed = fx.listed().await;

    assert_eq!(
        listed.iter().map(|item| item.reason).collect::<Vec<_>>(),
        [EnumValue::from(
            UnmatchedReason::UNMATCHED_REASON_UNSPECIFIED
        )]
    );
}

#[tokio::test]
async fn an_unmatched_item_is_listed_with_what_its_name_was_parsed_as_and_a_matched_one_is_not() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    let alien = "Alien (1979)/Alien (1979).mkv";
    fx.write(alien, VIDEO);
    fx.write(HOME_MOVIES, VIDEO);
    let home_movies = fx.scan_and_match().await[HOME_MOVIES];

    let listed = fx.listed().await;

    assert_eq!(
        (fx.movie_of(alien).await, fx.movie_of(HOME_MOVIES).await),
        (Some(tmdb_id(348)), None)
    );
    assert_eq!(
        listed,
        [unmatched_movie(
            home_movies,
            fx.path(HOME_MOVIES),
            "Home Movies",
            2019
        )]
    );
}

#[tokio::test]
async fn an_unmatched_item_matched_by_hand_drops_off_the_list() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];
    let before = fx.listed().await;

    fx.set_match(item, 949).await.unwrap();

    assert_eq!(before.len(), 1);
    assert_eq!(fx.listed().await, []);
}

#[tokio::test]
async fn an_episode_not_matched_yet_is_listed_with_its_season_and_number() {
    let fx = Fixture::start_without_metadata(LibraryKind::LIBRARY_KIND_SHOWS).await;
    let episode = "The Office/Season 01/The Office - S01E02.mkv";
    fx.write(episode, VIDEO);
    let item = fx.scan_and_match().await[episode];

    let listed = fx.listed().await;

    assert_eq!(
        listed,
        [UnmatchedItem {
            item_id: item,
            path: fx.path(episode),
            kind: TitleKind::TITLE_KIND_SHOW.into(),
            title: "The Office".to_owned(),
            year: None,
            season: Some(1),
            episode: Some(2),
            ..Default::default()
        }]
    );
}

#[tokio::test]
async fn an_item_pinned_unmatched_is_not_listed() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_MOVIES, VIDEO);
    let item = fx.scan_and_match().await[HOME_MOVIES];
    let before = fx.listed().await;

    fx.matches
        .clear_match(ClearMatchRequest {
            item_id: item,
            ..Default::default()
        })
        .await
        .unwrap();

    assert_eq!(before.len(), 1);
    assert_eq!(fx.listed().await, []);
}

#[tokio::test]
async fn listing_the_unmatched_items_of_a_library_that_does_not_exist_is_not_found() {
    let fx = Fixture::start(LibraryKind::LIBRARY_KIND_MOVIES).await;

    let refused = fx
        .matches
        .list_unmatched_items(ListUnmatchedItemsRequest {
            library_id: fx.library + 1,
            ..Default::default()
        })
        .await;

    assert_refused(
        refused,
        ErrorCode::NotFound,
        RefusalReason::REFUSAL_REASON_LIBRARY_NOT_FOUND,
    );
}

/// Too little to go on: no year and no ID hint.
const HOME_VIDEO: &str = "home video.mkv";

#[tokio::test]
async fn a_file_whose_name_does_not_parse_is_listed_by_path_and_one_holding_items_is_not() {
    let fx = Fixture::start_without_metadata(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_VIDEO, VIDEO);
    fx.write(HOME_MOVIES, VIDEO);
    fx.scan_and_match().await;

    let unparsed = fx.unparsed().await;

    assert_eq!(unparsed, [fx.path(HOME_VIDEO)]);
}

#[tokio::test]
async fn a_file_renamed_so_its_name_parses_drops_off_the_list_once_rescanned() {
    let fx = Fixture::start_without_metadata(LibraryKind::LIBRARY_KIND_MOVIES).await;
    fx.write(HOME_VIDEO, VIDEO);
    fx.scan_and_match().await;
    let before = fx.unparsed().await;

    fs::create_dir_all(fx.folder.join("Home Movies (2019)")).unwrap();
    fs::rename(fx.folder.join(HOME_VIDEO), fx.folder.join(HOME_MOVIES)).unwrap();
    fx.scan_and_match().await;

    assert_eq!(before, [fx.path(HOME_VIDEO)]);
    assert_eq!(fx.unparsed().await, Vec::<String>::new());
}
