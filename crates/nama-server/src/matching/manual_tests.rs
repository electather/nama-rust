use std::time::Duration;

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::{
    ManualError, Summary,
    tests::{Fixture, Outcome, matched},
};
use crate::library::{ImageRef, Picked, UnmatchedReason};

#[tokio::test]
async fn a_manual_match_survives_a_rescan_after_its_file_changes() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    // TMDB finds nothing for it.
    let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
    movies.write(home_movies);
    media.scan(movies).await;
    let unmatched = media.outcomes(movies).await;
    let item = media.item(movies, home_movies).await;

    media
        .matcher
        .set_match(item, &ExternalId::tmdb(949))
        .await
        .unwrap();
    let searches = media.searches();
    movies.replace(home_movies);
    media.scan(movies).await;

    assert_eq!(unmatched, [Outcome::Unmatched]);
    assert_eq!(media.outcomes(movies).await, [matched(949, "Heat")]);
    // The rescan followed the pin rather than searching again.
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_manual_match_replaces_an_automatic_one_for_every_file_named_alike() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies.write("Alien (1979)/Alien (1979).mkv");
    movies.write("Alien (1979)/Alien (1979) - Director's Cut.mkv");
    media.scan(movies).await;
    let item = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;

    let summary = media
        .matcher
        .set_match(item, &ExternalId::tmdb(949))
        .await
        .unwrap();

    assert_eq!(summary.matched, 2);
    assert_eq!(
        media.outcomes(movies).await,
        [matched(949, "Heat"), matched(949, "Heat")]
    );
}

#[tokio::test]
async fn re_matching_a_show_re_matches_its_episodes() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    // Both the US and the UK Office fit, so the scan leaves them Unmatched.
    shows.write("The Office/Season 01/The Office - S01E01.mkv");
    shows.write("The Office/Season 01/The Office - S01E02.mkv");
    media.scan(shows).await;
    let pilot = media
        .item(shows, "The Office/Season 01/The Office - S01E01.mkv")
        .await;

    media
        .matcher
        .set_match(pilot, &ExternalId::tmdb(2316))
        .await
        .unwrap();
    let us = media.outcomes(shows).await;
    media
        .matcher
        .set_match(pilot, &ExternalId::tmdb(2996))
        .await
        .unwrap();

    assert_eq!(
        us,
        [
            matched(2_316_001, "Pilot"),
            matched(2_316_002, "Diversity Day")
        ]
    );
    assert_eq!(
        media.outcomes(shows).await,
        [
            matched(2_996_001, "Downsize"),
            matched(2_996_002, "Work Experience")
        ]
    );
}

#[tokio::test]
async fn a_new_episode_of_a_pinned_show_matches_the_pinned_show() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write("The Office/Season 01/The Office - S01E01.mkv");
    media.scan(shows).await;
    let pilot = media
        .item(shows, "The Office/Season 01/The Office - S01E01.mkv")
        .await;
    media
        .matcher
        .set_match(pilot, &ExternalId::tmdb(2316))
        .await
        .unwrap();

    shows.write("The Office/Season 01/The Office - S01E02.mkv");
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await[1],
        matched(2_316_002, "Diversity Day")
    );
}

#[tokio::test]
async fn a_match_fixed_while_its_librarys_scan_lock_is_held_waits_then_applies() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let alien = "Alien (1979)/Alien (1979).mkv";
    movies.write(alien);
    media.scan(movies).await;
    let item = media.item(movies, alien).await;
    let scan = media.scanner.lock(movies.id).await;

    let matcher = media.matcher.clone();
    let mut clearing = tokio::spawn(async move { matcher.clear_match(item).await });
    let waited = tokio::time::timeout(Duration::from_millis(100), &mut clearing).await;
    assert!(waited.is_err(), "the Match was fixed during the scan");
    let during_scan = media.outcomes(movies).await;
    drop(scan);
    let cleared = clearing.await.unwrap().unwrap();

    assert_eq!(during_scan, [matched(348, "Alien")]);
    assert_eq!(
        cleared,
        Summary {
            unmatched: 1,
            ..Summary::default()
        }
    );
    assert_eq!(media.outcomes(movies).await, [Outcome::Unmatched]);
}

#[tokio::test]
async fn an_episode_matched_to_a_show_without_its_season_is_unmatched_as_not_in_show() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    // The US Office has only Season 1 at TMDB.
    let episode = "The Office/Season 10/The Office - S10E01.mkv";
    shows.write(episode);
    media.scan(shows).await;
    let item = media.item(shows, episode).await;

    let summary = media
        .matcher
        .set_match(item, &ExternalId::tmdb(2316))
        .await
        .unwrap();

    assert_eq!(
        summary,
        Summary {
            matched: 0,
            unmatched: 1,
            undecided: 0
        }
    );
    assert_eq!(
        media.reasons(shows).await,
        [Some(UnmatchedReason::NotInShow)]
    );
}

#[tokio::test]
async fn an_episode_matched_to_a_show_whose_season_lacks_it_is_unmatched_as_not_in_show() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    let episode = "The Office/Season 01/The Office - S01E42.mkv";
    shows.write(episode);
    media.scan(shows).await;
    let item = media.item(shows, episode).await;

    let summary = media
        .matcher
        .set_match(item, &ExternalId::tmdb(2316))
        .await
        .unwrap();

    assert_eq!(
        summary,
        Summary {
            matched: 0,
            unmatched: 1,
            undecided: 0
        }
    );
    assert_eq!(
        media.reasons(shows).await,
        [Some(UnmatchedReason::NotInShow)]
    );
}

#[tokio::test]
async fn a_cleared_match_stays_unmatched_after_its_file_changes() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let alien = "Alien (1979)/Alien (1979).mkv";
    movies.write(alien);
    media.scan(movies).await;
    let item = media.item(movies, alien).await;

    media.matcher.clear_match(item).await.unwrap();
    let cleared = media.outcomes(movies).await;
    movies.replace(alien);
    media.scan(movies).await;

    assert_eq!(cleared, [Outcome::Unmatched]);
    assert_eq!(media.outcomes(movies).await, [Outcome::Unmatched]);
}

#[tokio::test]
async fn a_match_to_a_title_the_source_does_not_have_is_refused_and_changes_nothing() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies.write("Alien (1979)/Alien (1979).mkv");
    media.scan(movies).await;
    let item = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;

    let result = media.matcher.set_match(item, &ExternalId::tmdb(1)).await;

    assert!(
        matches!(result, Err(ManualError::NoSuchTitle(ref id)) if *id == ExternalId::tmdb(1)),
        "{result:?}"
    );
    assert_eq!(media.outcomes(movies).await, [matched(348, "Alien")]);
}

#[tokio::test]
async fn a_match_for_an_item_that_does_not_exist_is_refused() {
    let media = Fixture::new().await;

    let result = media.matcher.set_match(42, &ExternalId::tmdb(949)).await;

    assert!(
        matches!(result, Err(ManualError::NoSuchItem(42))),
        "{result:?}"
    );
}

#[tokio::test]
async fn a_match_set_while_no_metadata_provider_is_enabled_is_unavailable() {
    let media = Fixture::without_tmdb().await;
    media.movies.write("Alien (1979)/Alien (1979).mkv");
    media.scan(&media.movies).await;
    let item = media
        .item(&media.movies, "Alien (1979)/Alien (1979).mkv")
        .await;

    let result = media.matcher.set_match(item, &ExternalId::tmdb(348)).await;

    assert!(
        matches!(result, Err(ManualError::Unavailable)),
        "{result:?}"
    );
}

fn picked_poster(path: Option<&str>) -> Picked {
    let fanart = PluginId::from_static("fanart");
    let poster = path.and_then(|path| ImageRef::of(&fanart, Some(&path.to_owned())));
    Picked {
        poster,
        ..Picked::default()
    }
}

fn episode_artwork(season_poster: Option<&str>, picked: Option<&str>) -> super::Metadata {
    use crate::library::{SourceEpisode, SourceSeason, SourceShow};
    let show = SourceShow {
        ids: ExternalIds::default(),
        title: "Severance".into(),
        original_title: None,
        overview: None,
        first_air_date: None,
        poster: Some("/show.jpg".into()),
        backdrop: Some("/show-backdrop.jpg".into()),
        seasons: vec![1],
        genres: Vec::new(),
        content_rating: None,
    };
    let episode = SourceEpisode {
        ids: ExternalIds::default(),
        season: 1,
        number: 1,
        title: "Good News About Hell".into(),
        overview: None,
        air_date: None,
        runtime_minutes: None,
        still: None,
    };
    let season = SourceSeason {
        ids: ExternalIds::default(),
        number: 1,
        title: "Season 1".into(),
        overview: None,
        air_date: None,
        poster: season_poster.map(Into::into),
        episodes: Vec::new(),
    };
    let tmdb = &PluginId::from_static("tmdb");
    super::Metadata::episode(tmdb, (&show, &picked_poster(picked)), &season, &episode)
}

#[test]
fn an_episode_keeps_its_season_poster_over_the_picked_show_poster() {
    let poster = episode_artwork(Some("/season.jpg"), Some("/picked.jpg")).poster;
    assert_eq!(poster.unwrap().path, "/season.jpg");
}

#[test]
fn an_episode_without_a_season_poster_takes_the_picked_show_poster() {
    let poster = episode_artwork(None, Some("/picked.jpg")).poster;
    assert_eq!(poster, picked_poster(Some("/picked.jpg")).poster);
}

#[test]
fn an_episode_with_no_season_or_picked_poster_takes_the_show_poster() {
    let poster = episode_artwork(None, None).poster;
    assert_eq!(poster.unwrap().path, "/show.jpg");
}

#[test]
fn a_shows_picked_poster_wins_over_its_own_and_keeps_its_own_backdrop() {
    let own = episode_artwork(None, None);
    let picked = picked_poster(Some("/picked.jpg"));
    let with = own.clone().with_picked(&picked);
    assert_eq!((with.poster, with.backdrop), (picked.poster, own.backdrop));
}
