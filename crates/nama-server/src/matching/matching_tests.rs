use std::{collections::BTreeMap, sync::Arc};

use futures_util::{FutureExt, StreamExt};
use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds, IMDB},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::Tmdb;
use nama_proto::nama::v1::{EventType, ItemMatched, subscribe_response};

use super::testing::{Fixture, Outcome, episode, hinted, matched, movie};
use crate::{
    events::{Filter, event},
    library::{Decided, UnmatchedReason},
};
#[tokio::test]
async fn a_scan_matches_movies_confidently_and_leaves_the_rest_unmatched() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let files = [
        ("Alien (1979)/Alien (1979).mkv", vec![movie("Alien", 1979)]),
        ("Dune (2021)/Dune (2021).mkv", vec![movie("Dune", 2021)]),
        // An ID hint is used directly: there is no recorded search for Heat.
        (
            "Heat (1995) {tmdb-949}/Heat (1995).mkv",
            vec![hinted(movie("Heat", 1995), ExternalId::tmdb(949))],
        ),
        // Two films called Hamlet came out in 2000; one has over ten times the other's votes.
        (
            "Hamlet (2000)/Hamlet (2000).mkv",
            vec![movie("Hamlet", 2000)],
        ),
        // TMDB finds nothing.
        (
            "Home Movies (2019)/Home Movies (2019).mkv",
            vec![movie("Home Movies", 2019)],
        ),
        // The ID hint names no title on TMDB.
        (
            "Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv",
            vec![hinted(movie("Lost Reel", 1950), ExternalId::tmdb(1))],
        ),
        // No title or year to search by: no item, so nothing to Match.
        ("home video.mkv", vec![]),
    ];
    for (path, items) in files {
        movies.write(path, &items).await;
    }

    media.scan(movies).await;

    assert_eq!(
        media.stored(movies).await,
        [
            (
                "Alien (1979)/Alien (1979).mkv".to_owned(),
                None,
                matched(348, "Alien")
            ),
            (
                "Dune (2021)/Dune (2021).mkv".to_owned(),
                None,
                matched(438_631, "Dune")
            ),
            (
                "Hamlet (2000)/Hamlet (2000).mkv".to_owned(),
                None,
                matched(10_688, "Hamlet")
            ),
            (
                "Heat (1995) {tmdb-949}/Heat (1995).mkv".to_owned(),
                None,
                matched(949, "Heat")
            ),
            (
                "Home Movies (2019)/Home Movies (2019).mkv".to_owned(),
                None,
                Outcome::Unmatched
            ),
            (
                "Lost Reel (1950) {tmdb-1}/Lost Reel (1950).mkv".to_owned(),
                None,
                Outcome::Unmatched
            ),
        ]
    );
    let alien = media.first_recorded(movies).await;
    assert_eq!(alien.plugin.as_deref(), Some("tmdb"));
    assert_eq!(
        alien.ids,
        ExternalIds::from_iter([
            ExternalId::tmdb(348),
            ExternalId::new(IMDB, "tt0078748").unwrap(),
        ])
    );
    assert_eq!(
        alien.poster.as_deref(),
        Some("/vfrQk5IPloGg1v9Rzbh2Eg3VGyM.jpg")
    );
}

#[tokio::test]
async fn tvdb_and_imdb_id_hints_match_without_a_search() {
    let media = Fixture::new().await;
    let (movies, shows) = (&media.movies, &media.shows);
    let (alien, dark) = (
        ExternalId::new(IMDB, "tt0078748").unwrap(),
        ExternalId::tvdb(334_824),
    );
    movies
        .write(
            "Alien {imdb-tt0078748}/Alien.mkv",
            &[hinted(movie("Alien", None), alien.clone())],
        )
        .await;
    movies
        .write(
            "Alien (1979) tt0078748/Alien (1979).mkv",
            &[hinted(movie("Alien", 1979), alien.clone())],
        )
        .await;
    // TMDB knows no title with this TVDB id.
    movies
        .write(
            "Lost Reel (1950) [tvdbid-1]/Lost Reel (1950).mkv",
            &[hinted(movie("Lost Reel", 1950), ExternalId::tvdb(1))],
        )
        .await;
    shows
        .write(
            "Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv",
            &[hinted(episode("Dark", None, 1, 1), dark.clone())],
        )
        .await;
    shows
        .write(
            "Dark {tvdb-334824}/Dark - S01E02.mkv",
            &[hinted(episode("Dark", None, 1, 2), dark)],
        )
        .await;
    // The IMDb id names a Movie, not a Show.
    shows
        .write(
            "Alien (imdb=tt0078748)/Season 01/Alien - S01E01.mkv",
            &[hinted(episode("Alien", None, 1, 1), alien)],
        )
        .await;

    media.scan(movies).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(movies).await,
        [
            matched(348, "Alien"),
            matched(348, "Alien"),
            Outcome::Unmatched
        ]
    );
    assert_eq!(
        media.outcomes(shows).await,
        [
            Outcome::Unmatched,
            matched(1_388_538, "Secrets"),
            matched(1_388_539, "Lies"),
        ]
    );
    assert_eq!(media.searches(), 0, "{:?}", media.tmdb.requests());
}

#[tokio::test]
async fn a_newer_name_parser_matches_an_item_by_its_tvdb_id_hint() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    let pilot = "Dark [tvdbid-334824]/Season 01/Dark - S01E01.mkv";
    // Before TVDB ids were read, the folder named a Show with no year and no ID Hint, which
    // a search left Unmatched.
    shows.write(pilot, &[episode("Dark", None, 1, 1)]).await;
    let item = media.item(shows, pilot).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    assert_eq!(media.items(shows).await[0].1.item.name().hint, None);

    let tvdb = ExternalId::tvdb(334_824);
    shows
        .reparse(pilot, &[hinted(episode("Dark", None, 1, 1), tvdb)])
        .await;
    media.scan(shows).await;

    assert_eq!(media.outcomes(shows).await, [matched(1_388_538, "Secrets")]);
}

#[tokio::test]
async fn a_match_is_published_once_stored() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    // Unmatched, which publishes nothing.
    movies
        .write(
            "Home Movies (2019)/Home Movies (2019).mkv",
            &[movie("Home Movies", 2019)],
        )
        .await;
    let filter = Filter {
        types: vec![EventType::EVENT_TYPE_ITEM_MATCHED],
        ..Filter::default()
    };
    let mut matches = Box::pin(media.events.subscribe_as_admin(filter));

    media.scan(movies).await;

    let alien = media.item(movies, "Alien (1979)/Alien (1979).mkv").await;
    let expected = event(ItemMatched {
        library_id: movies.id,
        item_id: alien,
        title: "Alien".to_owned(),
        ..Default::default()
    });
    assert_eq!(
        matches.next().await.and_then(|response| response.message),
        Some(subscribe_response::Message::from(expected))
    );
    assert!(
        matches.next().now_or_never().is_none(),
        "an Unmatched item was published"
    );
}

#[tokio::test]
async fn a_scan_matches_episodes_through_their_show_and_season() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows
        .write(
            "Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv",
            &[episode("Dark", 2017, 1, 1), episode("Dark", 2017, 1, 2)],
        )
        .await;
    shows
        .write(
            "Dark (2017)/Season 01/Dark (2017) - S01E03.mkv",
            &[episode("Dark", 2017, 1, 3)],
        )
        .await;
    // Season 1 has no Episode 42.
    shows
        .write(
            "Dark (2017)/Season 01/Dark (2017) - S01E42.mkv",
            &[episode("Dark", 2017, 1, 42)],
        )
        .await;
    // No year, and both the US and the UK Office fit.
    shows
        .write(
            "The Office/Season 01/The Office - S01E01.mkv",
            &[episode("The Office", None, 1, 1)],
        )
        .await;

    media.scan(shows).await;

    let dark = "Dark (2017)/Season 01/Dark (2017)";
    assert_eq!(
        media.stored(shows).await,
        [
            (
                format!("{dark} - S01E01-E02.mkv"),
                Some((1, 1)),
                matched(1_388_538, "Secrets")
            ),
            (
                format!("{dark} - S01E01-E02.mkv"),
                Some((1, 2)),
                matched(1_388_539, "Lies")
            ),
            (
                format!("{dark} - S01E03.mkv"),
                Some((1, 3)),
                matched(1_388_540, "Past and Present")
            ),
            (
                format!("{dark} - S01E42.mkv"),
                Some((1, 42)),
                Outcome::Unmatched
            ),
            (
                "The Office/Season 01/The Office - S01E01.mkv".to_owned(),
                Some((1, 1)),
                Outcome::Unmatched
            ),
        ]
    );
    let not_in_show = (
        format!("{dark} - S01E42.mkv"),
        Some(UnmatchedReason::NotInShow),
    );
    assert!(media.reasons(shows).await.contains(&not_in_show));
    let secrets = media.first_recorded(shows).await;
    assert_eq!(secrets.show_title.as_deref(), Some("Dark"));
    assert_eq!(
        secrets.show_ids,
        ExternalIds::from_iter([
            ExternalId::tmdb(70523),
            ExternalId::new(IMDB, "tt5753856").unwrap(),
            ExternalId::tvdb(334_824),
        ])
    );
    // The Show and its Season were each fetched once.
    let requests = media.tmdb.requests();
    let count = |prefix: &str| requests.iter().filter(|r| r.starts_with(prefix)).count();
    assert_eq!(count("/3/tv/70523?"), 1, "{requests:?}");
    assert_eq!(count("/3/tv/70523/season/1?"), 1, "{requests:?}");
}

#[tokio::test]
async fn a_country_tag_on_a_show_is_searched_without_and_keeps_shows_from_that_country() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    // TMDB also has a Euphoria with no country, which is dropped.
    shows
        .write(
            "Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv",
            &[episode("Euphoria (US)", None, 1, 1)],
        )
        .await;
    // TheTVDB's UK is TMDB's GB.
    shows
        .write(
            "The Office (UK)/Season 01/The Office (UK) - S01E01.mkv",
            &[episode("The Office (UK)", None, 1, 1)],
        )
        .await;
    shows
        .write(
            "The Office (US)/Season 01/The Office (US) - S01E01.mkv",
            &[episode("The Office (US)", None, 1, 1)],
        )
        .await;

    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [
            matched(1_734_395, "Pilot"),
            matched(2_996_001, "Downsize"),
            matched(2_316_001, "Pilot"),
        ]
    );
}

#[tokio::test]
async fn a_show_left_unmatched_by_the_parser_before_country_tags_matches_once_rescanned() {
    let media = Fixture::without_tmdb().await;
    let shows = &media.shows;
    let pilot = "Euphoria (US)/Season 01/Euphoria (US) - S01E01.mkv";
    let pilot_items = [episode("Euphoria (US)", None, 1, 1)];
    shows.write(pilot, &pilot_items).await;
    media.scan(shows).await;
    let item = media.item(shows, pilot).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    shows.reparse(pilot, &pilot_items).await;
    media.enable_tmdb();

    media.scan(shows).await;

    assert_eq!(media.outcomes(shows).await, [matched(1_734_395, "Pilot")]);
}

#[tokio::test]
async fn a_bracket_that_is_not_a_country_stays_in_the_shows_title() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    shows
        .write(
            "The Office (II)/Season 01/The Office (II) - S01E01.mkv",
            &[episode("The Office (II)", None, 1, 1)],
        )
        .await;

    media.scan(shows).await;

    let requests = media.tmdb.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("/3/search/tv?") && r.contains("query=The+Office+%28II%29")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn a_country_tag_in_a_movies_name_stays_in_its_title() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    movies
        .write(
            "Euphoria (US) (2019)/Euphoria (US) (2019).mkv",
            &[movie("Euphoria (US)", 2019)],
        )
        .await;

    media.scan(movies).await;

    let requests = media.tmdb.requests();
    assert!(
        requests
            .iter()
            .any(|r| r.starts_with("/3/search/movie?") && r.contains("query=Euphoria+%28US%29")),
        "{requests:?}"
    );
}

#[tokio::test]
async fn a_pin_on_a_show_named_with_a_country_tag_still_applies_after_a_newer_parser() {
    let media = Fixture::new().await;
    let shows = &media.shows;
    let pilot = "The Office (US)/Season 01/The Office (US) - S01E01.mkv";
    let pilot_items = [episode("The Office (US)", None, 1, 1)];
    shows.write(pilot, &pilot_items).await;
    media.scan(shows).await;
    let item = media.item(shows, pilot).await;
    media
        .matcher
        .set_match(item, &ExternalId::tmdb(2996))
        .await
        .unwrap();
    let searches = media.searches();

    shows.reparse(pilot, &pilot_items).await;
    media.scan(shows).await;

    assert_eq!(
        media.outcomes(shows).await,
        [matched(2_996_001, "Downsize")]
    );
    assert_eq!(media.searches(), searches);
}

#[tokio::test]
async fn a_year_off_by_one_still_matches_though_tmdb_filters_by_exact_year() {
    let media = Fixture::new().await;
    // Released in 1979 in the US, but 1980 where this copy came from. TMDB finds no Alien
    // from 1980, so the search is tried again without the year.
    media
        .movies
        .write("Alien (1980)/Alien (1980).mkv", &[movie("Alien", 1980)])
        .await;

    media.scan(&media.movies).await;

    assert_eq!(
        media.stored(&media.movies).await[0].2,
        matched(348, "Alien")
    );
}

#[tokio::test]
async fn a_failing_metadata_provider_stops_the_pass_and_leaves_items_undecided() {
    let media = Fixture::without_tmdb().await;
    let settings = Settings::new(BTreeMap::from([(
        "api_key".to_owned(),
        SettingValue::Text("rejected".to_owned()),
    )]));
    media
        .plugins
        .enable_with_settings(
            Arc::new(Tmdb::with_endpoints(media.tmdb.endpoints())),
            settings,
        )
        .unwrap();
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    media
        .movies
        .write("Dune (2021)/Dune (2021).mkv", &[movie("Dune", 2021)])
        .await;

    media.scan(&media.movies).await;

    // TMDB rejected the key once; nothing more was asked.
    assert_eq!(media.tmdb.requests().len(), 1);
    assert_eq!(
        media
            .stored(&media.movies)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect::<Vec<_>>(),
        [Outcome::Undecided, Outcome::Undecided]
    );
}

#[tokio::test]
async fn a_rescan_does_not_look_up_items_already_decided() {
    let media = Fixture::new().await;
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;
    media
        .movies
        .write(
            "Home Movies (2019)/Home Movies (2019).mkv",
            &[movie("Home Movies", 2019)],
        )
        .await;
    media.scan(&media.movies).await;
    let looked_up = media.lookups();

    media.scan(&media.movies).await;

    assert_eq!(media.lookups(), looked_up);
    assert_eq!(
        media
            .stored(&media.movies)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect::<Vec<_>>(),
        [matched(348, "Alien"), Outcome::Unmatched]
    );
}

#[tokio::test]
async fn a_newer_name_parser_retries_unmatched_items_and_keeps_automatic_matches() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let (alien_path, alien_items) = ("Alien (1979)/Alien (1979).mkv", [movie("Alien", 1979)]);
    let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
    let home_movies_items = [movie("Home Movies", 2019)];
    movies.write(alien_path, &alien_items).await;
    movies.write(home_movies, &home_movies_items).await;
    media.scan(movies).await;
    let alien = media.item(movies, alien_path).await;
    let searches = media.searches();

    movies.reparse(alien_path, &alien_items).await;
    movies.reparse(home_movies, &home_movies_items).await;
    media.scan(movies).await;

    // Only Home Movies, left Unmatched, was searched for again: with its year, then without.
    assert_eq!(media.searches(), searches + 2);
    assert_eq!(
        media.outcomes(movies).await,
        [matched(348, "Alien"), Outcome::Unmatched]
    );
    assert_eq!(media.item(movies, alien_path).await, alien);
}

#[tokio::test]
async fn a_newer_name_parser_matches_an_item_an_older_one_left_ambiguous() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let hamlet = "Hamlet (2000)/Hamlet (2000).mkv";
    let hamlet_items = [movie("Hamlet", 2000)];
    movies.write(hamlet, &hamlet_items).await;
    media.scan(movies).await;
    // Before votes broke a tie, the two Hamlets of 2000 left this file Unmatched.
    let item = media.item(movies, hamlet).await;
    media
        .contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();

    movies.reparse(hamlet, &hamlet_items).await;
    media.scan(movies).await;

    assert_eq!(media.outcomes(movies).await, [matched(10_688, "Hamlet")]);
}

#[tokio::test]
async fn a_newer_name_parser_keeps_a_pinned_match() {
    let media = Fixture::new().await;
    let movies = &media.movies;
    let home_movies = "Home Movies (2019)/Home Movies (2019).mkv";
    let home_movies_items = [movie("Home Movies", 2019)];
    movies.write(home_movies, &home_movies_items).await;
    media.scan(movies).await;
    let item = media.item(movies, home_movies).await;
    media
        .matcher
        .set_match(item, &ExternalId::tmdb(949))
        .await
        .unwrap();
    let looked_up = media.lookups();

    movies.reparse(home_movies, &home_movies_items).await;
    media.scan(movies).await;

    assert_eq!(media.outcomes(movies).await, [matched(949, "Heat")]);
    assert_eq!(media.lookups(), looked_up);
}

#[tokio::test]
async fn items_stay_undecided_until_a_metadata_provider_can_match_them() {
    let media = Fixture::without_tmdb().await;
    media
        .movies
        .write("Alien (1979)/Alien (1979).mkv", &[movie("Alien", 1979)])
        .await;

    media.scan(&media.movies).await;
    let before = media.stored(&media.movies).await;
    media.enable_tmdb();
    media.scan(&media.movies).await;

    assert_eq!(before[0].2, Outcome::Undecided);
    assert_eq!(
        media.stored(&media.movies).await[0].2,
        matched(348, "Alien")
    );
    assert!(
        media
            .plugins
            .enabled()
            .contains(&PluginId::from_static("tmdb"))
    );
}
