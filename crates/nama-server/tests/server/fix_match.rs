//! Fixing a Matched Movie's or Show's Match by hand, as its page does, through the generated
//! `MatchService` client, over a real socket, with the TMDB Plugin enabled against a fake TMDB.

use crate::support::{browsing, refused, server};

use std::{sync::Arc, time::Duration};

use browsing::{Fixture, movies_in, tmdb_id};
use connectrpc::{ErrorCode, client::HttpClient};
use nama_plugin_upkeep::Upkeep;
use nama_proto::nama::v1::{
    ClearMatchRequest, JobRunOutcome, JobServiceClient, LibraryKind, ListEpisodesRequest,
    ListJobRunsRequest, ListUnmatchedItemsRequest, MatchedTitle, RefusalReason, RunJobRequest,
    SearchTitlesRequest, SetMatchRequest, TitleKind, UnmatchedItemFilter, UnmatchedReason,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::TestServer;

/// Two files of Alien named differently, so each name is pinned on its own.
const ALIEN: &str = "Alien (1979)/Alien (1979).mkv";
const ALIEN_CUT: &str = "Alien (1979)/Alien {tmdb-348}.mkv";

/// Two Episodes of Dark in folders named differently.
const DARK_E01: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
const DARK_E02: &str = "Dark {tmdb-70523}/Season 01/Dark - S01E02.mkv";

/// A Server with TMDB and Upkeep, and a Library of `kind` holding `files`, scanned and Matched.
async fn start(kind: LibraryKind, files: &[&str]) -> Fixture {
    let plugins = Registry::new();
    plugins.describe_builtin(|| Arc::new(Upkeep));
    let fx = Fixture::start_on(kind, TestServer::builder().plugins(plugins)).await;
    for file in files {
        fx.write(file, b"video");
    }
    fx.scan_and_match().await;
    fx
}

/// The title of the Library of `fx` with the TMDB id `tmdb`, as Fix Match names it.
fn title(fx: &Fixture, tmdb: i64) -> MatchedTitle {
    MatchedTitle {
        library_id: fx.library,
        id: tmdb_id(tmdb).into(),
        ..Default::default()
    }
}

/// Runs Upkeep's weekly `refresh-metadata` Job now, as an Admin can, and waits for its Run to
/// succeed, since `RunJob` answers once the Run started.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn refresh_metadata(fx: &Fixture) {
    const JOB: &str = "refresh-metadata";
    let jobs = JobServiceClient::new(HttpClient::plaintext(), fx.server.client_config());
    let start = RunJobRequest {
        plugin_id: "upkeep".to_owned(),
        job_id: JOB.to_owned(),
        ..Default::default()
    };
    jobs.run_job(start).await.unwrap();
    let ended = async {
        loop {
            let runs = (jobs.list_job_runs(ListJobRunsRequest {
                plugin_id: "upkeep".to_owned(),
                job_id: JOB.to_owned(),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_owned()
            .runs;
            match runs.first().and_then(|run| run.outcome.as_known()) {
                Some(JobRunOutcome::JOB_RUN_OUTCOME_RUNNING) | None => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Some(outcome) => return outcome,
            }
        }
    };
    let outcome = tokio::time::timeout(Duration::from_secs(30), ended)
        .await
        .unwrap();
    assert_eq!(outcome, JobRunOutcome::JOB_RUN_OUTCOME_SUCCEEDED);
}

/// Each Movie of the Library of `fx`, by TMDB id, with how many Versions it has.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn movies(fx: &Fixture) -> Vec<(String, usize)> {
    let movies = movies_in(&fx.browse, fx.library).await.unwrap();
    (movies.into_iter())
        .map(|movie| (movie.id.value.clone(), movie.versions.len()))
        .collect()
}

/// The TMDB ids of the Episodes of Season 1 of the Show `show` in the Library of `fx`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn season_1(fx: &Fixture, show: i64) -> Vec<String> {
    let listed = (fx.browse)
        .list_episodes(ListEpisodesRequest {
            library_id: fx.library,
            show_id: tmdb_id(show).into(),
            season_number: Some(1),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    (listed.episodes.into_iter())
        .map(|episode| episode.id.value.clone())
        .collect()
}

#[tokio::test]
async fn fixing_a_movies_match_moves_every_version_and_holds_once_rescanned() {
    let fx = start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN, ALIEN_CUT]).await;
    let before = movies(&fx).await;

    let fixed = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: title(&fx, 348).into(),
            title_id: tmdb_id(949).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let after = movies(&fx).await;
    fx.write(ALIEN, b"a better copy of the video");
    fx.scan_and_match().await;

    assert_eq!(before, [("348".to_owned(), 2)]);
    assert_eq!((fixed.matched, fixed.unmatched), (2, 0));
    assert_eq!(after, [("949".to_owned(), 2)]);
    assert_eq!(movies(&fx).await, after);
}

#[tokio::test]
async fn fixing_a_shows_match_moves_every_episode_and_holds_once_refreshed_and_rescanned() {
    let fx = start(LibraryKind::LIBRARY_KIND_SHOWS, &[DARK_E01, DARK_E02]).await;
    let before = season_1(&fx, 70_523).await;

    let fixed = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: title(&fx, 70_523).into(),
            title_id: tmdb_id(2316).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let after = season_1(&fx, 2316).await;
    refresh_metadata(&fx).await;
    let refreshed = season_1(&fx, 2316).await;
    fx.write(DARK_E01, b"a better copy of the video");
    fx.scan_and_match().await;

    assert_eq!(before.len(), 2);
    assert_eq!(fixed.matched, 2);
    assert_eq!(after, ["2316001", "2316002"]);
    assert_eq!(refreshed, after);
    assert_eq!(season_1(&fx, 2316).await, after);
}

#[tokio::test]
async fn unmatching_a_movie_lists_every_version_as_cleared_by_hand() {
    let fx = start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN, ALIEN_CUT]).await;

    let cleared = (fx.matches)
        .clear_match(ClearMatchRequest {
            matched_title: title(&fx, 348).into(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let listed = (fx.matches)
        .list_unmatched_items(ListUnmatchedItemsRequest {
            filter: UnmatchedItemFilter {
                library_id: fx.library,
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
    assert_eq!(movies(&fx).await, []);
}

#[tokio::test]
async fn fixing_the_match_of_a_title_the_library_lacks_is_not_found() {
    let fx = start(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN]).await;

    let refused = (fx.matches)
        .set_match(SetMatchRequest {
            matched_title: title(&fx, 949).into(),
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
    let fx = start(LibraryKind::LIBRARY_KIND_MOVIES, &[]).await;
    let search = |library_id| SearchTitlesRequest {
        kind: TitleKind::TITLE_KIND_MOVIE.into(),
        title: "Alien".to_owned(),
        year: Some(1979),
        library_id: Some(library_id),
        ..Default::default()
    };

    let found = (fx.matches.search_titles(search(fx.library)).await)
        .unwrap()
        .into_owned();
    let no_library = fx.matches.search_titles(search(fx.library + 1)).await;

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
