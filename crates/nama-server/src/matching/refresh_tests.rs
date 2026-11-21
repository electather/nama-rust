use nama_plugin_api::{PluginId, metadata::ExternalId};
use tokio_util::sync::CancellationToken;

use super::{
    Refreshed,
    testing::{Fixture, episode, matched, movie},
};
use crate::library::ImageRef;

const SECRETS: &str = "Dark (2017)/Season 01/Dark - S01E01.mkv";
/// TMDB has no Still for it.
const BEGINNINGS_AND_ENDINGS: &str = "Dark (2017)/Season 02/Dark - S02E01.mkv";
const SECRETS_STILL: &str = "/gNnJBvHy3VBkGrsl9ffpkf6ucP3.jpg";

fn refreshed(titles: u64, unavailable: &[PluginId]) -> Refreshed {
    let unavailable = unavailable.iter().cloned().collect();
    Refreshed {
        titles,
        unavailable,
    }
}

#[tokio::test]
async fn a_match_stored_at_an_older_metadata_version_gets_its_still_on_the_next_scan() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    shows
        .write(BEGINNINGS_AND_ENDINGS, &[episode("Dark", 2017, 2, 1)])
        .await;
    media.scan(shows).await;
    let matched = media.outcomes(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    let searches = media.searches();

    media.scan(shows).await;

    assert_eq!(
        media.stills(shows).await,
        [Some(SECRETS_STILL.to_owned()), None]
    );
    assert_eq!(media.outcomes(shows).await, matched);
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_movie_stored_at_an_older_metadata_version_is_fetched_again_by_its_id() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    media.scan(movies).await;
    media.contents.stored_by_older_metadata(movies.id).await;
    let searches = media.searches();
    let movie_fetches = || {
        let requests = media.tmdb.requests().into_iter();
        requests.filter(|r| r.starts_with("/3/movie/348?")).count()
    };
    let before = movie_fetches();

    media.scan(movies).await;

    assert_eq!(media.outcomes(movies).await, [matched(348, "Alien")]);
    assert_eq!(media.searches(), searches);
    assert_eq!(movie_fetches(), before + 1);
    let alien = media
        .contents
        .movies(movies.id, None)
        .await
        .unwrap()
        .remove(0);
    assert_eq!(
        (alien.genres, alien.content_rating),
        (
            vec!["Horror".to_owned(), "Science Fiction".to_owned()],
            Some("R".to_owned())
        )
    );
}

#[tokio::test]
async fn a_show_stored_at_an_older_metadata_version_gets_its_own_details_on_the_next_scan() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    let older = media
        .contents
        .shows(shows.id, None)
        .await
        .unwrap()
        .remove(0);
    let searches = media.searches();

    media.scan(shows).await;

    let dark = media
        .contents
        .shows(shows.id, None)
        .await
        .unwrap()
        .remove(0);
    // Until then, it is shown by its first Season's Poster.
    let path = |poster: Option<&ImageRef>| poster.map(|it| it.path.clone());
    assert_eq!(
        path(older.poster()).as_deref(),
        Some("/5LoHuHWA4H8jElFlZDvsmU2n63b.jpg")
    );
    assert_eq!(
        path(dark.poster()).as_deref(),
        Some("/apbrbWs8M9lyOpJYU5WXrpFbk1Z.jpg")
    );
    assert!(
        dark.overview
            .is_some_and(|it| it.starts_with("A missing child"))
    );
    assert_eq!(dark.first_air_date.as_deref(), Some("2017-12-01"));
    assert_eq!(dark.genres, ["Crime", "Drama", "Mystery"]);
    assert_eq!(dark.content_rating.as_deref(), Some("TV-MA"));
    assert_eq!(
        (dark.score, dark.original_language.as_deref()),
        (Some(8.4), Some("de"))
    );
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_match_at_the_current_metadata_version_is_not_fetched_again() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    shows
        .write(BEGINNINGS_AND_ENDINGS, &[episode("Dark", 2017, 2, 1)])
        .await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    media.scan(shows).await;
    let looked_up = media.lookups();

    media.scan(shows).await;

    assert_eq!(media.lookups(), looked_up);
}

#[tokio::test]
async fn a_pinned_match_stored_at_an_older_metadata_version_stays_pinned_once_refreshed() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(shows).await;
    let item = media.item(shows, SECRETS).await;
    media
        .matcher
        .set_match(item, &ExternalId::tmdb(70523))
        .await
        .unwrap();
    media.contents.stored_by_older_metadata(shows.id).await;
    let searches = media.searches();

    media.scan(shows).await;
    let refreshed = media.stills(shows).await;
    // A changed file is Matched again, which follows a pin rather than searching.
    shows.replace(SECRETS).await;
    media.scan(shows).await;

    assert_eq!(refreshed, [Some(SECRETS_STILL.to_owned())]);
    assert_eq!(media.outcomes(shows).await, [matched(1_388_538, "Secrets")]);
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_match_the_provider_fails_to_refresh_stays_as_it_was_until_a_later_scan() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    let tmdb = PluginId::from_static("tmdb");
    media.plugins.disable(&tmdb).await;
    media.enable_tmdb_rejecting();

    media.scan(shows).await;
    let failed = media.stills(shows).await;
    let outcome = media.outcomes(shows).await;
    media.plugins.disable(&tmdb).await;
    media.scan(shows).await;
    let disabled = media.stills(shows).await;
    media.enable_tmdb();
    media.scan(shows).await;

    assert_eq!(failed, [None]);
    assert_eq!(outcome, [matched(1_388_538, "Secrets")]);
    assert_eq!(disabled, [None]);
    assert_eq!(media.stills(shows).await, [Some(SECRETS_STILL.to_owned())]);
}

#[tokio::test]
async fn a_match_from_a_gone_plugin_does_not_stop_others_being_refreshed() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    shows
        .write(BEGINNINGS_AND_ENDINGS, &[episode("Dark", 2017, 2, 1)])
        .await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    // Sorts before `tmdb`, so its title is asked about first.
    let gone = media.item(shows, BEGINNINGS_AND_ENDINGS).await;
    media.contents.matched_by(gone, "gone").await;

    media.scan(shows).await;

    assert_eq!(
        media.stills(shows).await,
        [Some(SECRETS_STILL.to_owned()), None]
    );
}

#[tokio::test]
async fn refreshing_every_match_fetches_each_title_again_by_its_id_whatever_its_metadata_version() {
    let media = Fixture::new().await;
    let (movies, shows) = (&media.movies, &media.shows);
    movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(movies).await;
    media.scan(shows).await;
    let searches = media.searches();
    let fetches = |path: &str| {
        let requests = media.tmdb.requests().into_iter();
        requests.filter(|r| r.starts_with(path)).count()
    };
    let before = (fetches("/3/movie/348?"), fetches("/3/tv/70523?"));

    let all = media.matcher.refresh_all(&CancellationToken::new()).await;

    assert_eq!(all.unwrap(), refreshed(2, &[]));
    assert_eq!(
        (fetches("/3/movie/348?"), fetches("/3/tv/70523?")),
        (before.0 + 1, before.1 + 1)
    );
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn refreshing_every_match_keeps_pinned_and_automatic_matches_on_their_titles() {
    let media = Fixture::new().await;
    let (movies, shows) = (&media.movies, &media.shows);
    movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(movies).await;
    media.scan(shows).await;
    let item = media.item(shows, SECRETS).await;
    let pin = ExternalId::tmdb(70523);
    media.matcher.set_match(item, &pin).await.unwrap();
    media.contents.stored_by_older_metadata(shows.id).await;
    let searches = media.searches();

    media
        .matcher
        .refresh_all(&CancellationToken::new())
        .await
        .unwrap();
    let refreshed = media.stills(shows).await;
    // A changed file is Matched again, which follows a pin rather than searching.
    shows.replace(SECRETS).await;
    media.scan(shows).await;

    assert_eq!(refreshed, [Some(SECRETS_STILL.to_owned())]);
    assert_eq!(media.outcomes(movies).await, [matched(348, "Alien")]);
    assert_eq!(media.outcomes(shows).await, [matched(1_388_538, "Secrets")]);
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_title_that_fails_to_refresh_keeps_its_metadata_and_the_next_refresh_tries_again() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    let tmdb = PluginId::from_static("tmdb");
    media.plugins.disable(&tmdb).await;
    media.enable_tmdb_rejecting();
    let cancel = CancellationToken::new();

    let failed = media.matcher.refresh_all(&cancel).await.unwrap();
    let stills = media.stills(shows).await;
    let outcome = media.outcomes(shows).await;
    media.plugins.disable(&tmdb).await;
    media.enable_tmdb();
    let retried = media.matcher.refresh_all(&cancel).await.unwrap();

    assert_eq!(failed, refreshed(0, &[tmdb]));
    assert_eq!(stills, [None]);
    assert_eq!(outcome, [matched(1_388_538, "Secrets")]);
    assert_eq!(retried, refreshed(1, &[]));
    assert_eq!(media.stills(shows).await, [Some(SECRETS_STILL.to_owned())]);
}

#[tokio::test]
async fn refreshing_every_match_skips_only_an_unavailable_sources_titles() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows.write(SECRETS, &[episode("Dark", 2017, 1, 1)]).await;
    shows
        .write(BEGINNINGS_AND_ENDINGS, &[episode("Dark", 2017, 2, 1)])
        .await;
    media.scan(shows).await;
    media.contents.stored_by_older_metadata(shows.id).await;
    // Sorts before `tmdb`, so its title is asked about first.
    let gone = media.item(shows, BEGINNINGS_AND_ENDINGS).await;
    media.contents.matched_by(gone, "gone").await;

    let all = media.matcher.refresh_all(&CancellationToken::new()).await;

    let unavailable = [PluginId::from_static("gone")];
    assert_eq!(all.unwrap(), refreshed(1, &unavailable));
    assert_eq!(
        media.stills(shows).await,
        [Some(SECRETS_STILL.to_owned()), None]
    );
}
