use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tempfile::TempDir;

use super::super::{
    Change, Decided, Item, Metadata,
    fixture::{MatchedMovie, decide, tmdb_movie},
    testing::Fixture,
};
use crate::library::libraries::Kind;

/// What TMDB says of the Movie `tmdb`, called `title` and released on `release_date`.
fn released_movie(tmdb: i64, title: &str, release_date: &str) -> Metadata {
    Metadata {
        release_date: Some(release_date.to_owned()),
        ..tmdb_movie(tmdb, title)
    }
}

impl Fixture {
    /// Stores a file of its own for the Movie `tmdb` and Matches it to `title`, released on
    /// `release_date`. Returns the Item's id.
    async fn match_movie(&self, tmdb: i64, title: &str, release_date: &str) -> i64 {
        let path = format!("/media/{tmdb}.mkv");
        let matched = released_movie(tmdb, title, release_date);
        MatchedMovie::store(&self.contents, self.library, &path, matched)
            .await
            .item
    }

    /// The titles of the Movies a search for `query` finds, best fit first.
    async fn found_movies(&self, query: &str) -> Vec<String> {
        let found = self.contents.search(query).await.unwrap();
        found.movies.into_iter().map(|movie| movie.title).collect()
    }
}

#[tokio::test]
async fn a_search_forgives_case_accents_and_punctuation() {
    let fx = Fixture::new().await;
    fx.match_movie(194, "Amélie", "2001-04-25").await;
    fx.match_movie(557, "Spider-Man", "2002-05-01").await;

    assert_eq!(fx.found_movies("AMELIE").await, ["Amélie"]);
    assert_eq!(fx.found_movies("spiderman").await, ["Spider-Man"]);
    assert_eq!(fx.found_movies("spider-man").await, ["Spider-Man"]);
}

#[tokio::test]
async fn a_year_after_the_words_narrows_a_search_to_that_year_or_titles_with_it() {
    let fx = Fixture::new().await;
    fx.match_movie(841, "Dune", "1984-12-14").await;
    fx.match_movie(438_631, "Dune", "2021-09-15").await;
    fx.match_movie(335_984, "Blade Runner 2049", "2017-10-04")
        .await;

    assert_eq!(fx.found_movies("dune").await, ["Dune", "Dune"]);
    let dune_2021 = fx.contents.search("dune 2021").await.unwrap().movies;
    let found: Vec<_> = dune_2021
        .iter()
        .map(|movie| movie.release_date.as_deref())
        .collect();
    assert_eq!(found, [Some("2021-09-15")]);
    assert_eq!(
        fx.found_movies("blade runner 2049").await,
        ["Blade Runner 2049"]
    );
    assert_eq!(fx.found_movies("2049").await, ["Blade Runner 2049"]);
}

/// What TMDB says of an Episode of The Office (US): its id `tmdb` and `title`.
fn office_episode(tmdb: i64, title: &str) -> Decided {
    Decided::Matched(Box::new(Metadata {
        plugin: PluginId::from_static("tmdb"),
        ids: ExternalIds::from_iter([ExternalId::tmdb(tmdb)]),
        show_ids: ExternalIds::from_iter([ExternalId::tmdb(THE_OFFICE)]),
        title: title.to_owned(),
        original_title: None,
        show_title: Some("The Office".to_owned()),
        overview: None,
        release_date: Some("2005-03-24".to_owned()),
        runtime_minutes: None,
        poster: None,
        backdrop: None,
        logo: None,
        still: None,
        genres: Vec::new(),
        content_rating: None,
        show_poster: None,
        show_overview: None,
        show_first_air_date: None,
    }))
}

const THE_OFFICE: i64 = 2316;

impl Fixture {
    /// A new Shows Library, and the folder it watches, which it lives no longer than.
    async fn shows_library(&self) -> (i64, TempDir) {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let library = self
            .libraries
            .create(
                "Shows",
                Kind::Shows,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        (library, folder)
    }

    /// Stores a file of its own in `library` for Episode `number` of Season 1 of The Office,
    /// and decides it as `decided`.
    async fn decide_office_episode(&self, library: i64, number: i64, decided: Decided) {
        let path = format!("/shows/The Office - S01E0{number}.mkv");
        let file = Change::Store {
            path: path.clone(),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Episode {
                show: "The Office".to_owned(),
                year: None,
                hint: None,
                season: 1,
                episode: number,
            }],
        };
        self.contents.apply(library, vec![file]).await.unwrap();
        decide(&self.contents, library, &path, vec![decided]).await;
    }
}

#[tokio::test]
async fn a_search_finds_a_show_once_by_its_title_and_episodes_by_their_own() {
    let fx = Fixture::new().await;
    let (library, _folder) = fx.shows_library().await;
    for (number, tmdb, title) in [(1, 9001, "Pilot"), (2, 9002, "Office Olympics")] {
        fx.decide_office_episode(library, number, office_episode(tmdb, title))
            .await;
    }

    let found = fx.contents.search("office").await.unwrap();

    let shows: Vec<_> = found
        .shows
        .iter()
        .map(|show| (show.library, show.title.as_str()))
        .collect();
    assert_eq!(shows, [(library, "The Office")]);
    let found_episodes: Vec<_> = found
        .episodes
        .iter()
        .map(|episode| {
            (
                &episode.show,
                episode.season,
                episode.number,
                episode.title.as_str(),
            )
        })
        .collect();
    assert_eq!(
        found_episodes,
        [(&ExternalId::tmdb(THE_OFFICE), 1, 2, "Office Olympics")]
    );
    assert!(found.movies.is_empty(), "{:?}", found.movies);
}

#[tokio::test]
async fn a_search_finds_an_item_by_its_latest_match_and_not_once_it_is_unmatched() {
    let fx = Fixture::new().await;
    let item = fx.match_movie(949, "Heat", "1995-12-15").await;

    let ronin = Decided::Matched(Box::new(released_movie(8195, "Ronin", "1998-09-25")));
    fx.contents.record(vec![(item, ronin)]).await.unwrap();

    assert_eq!(fx.found_movies("heat").await, Vec::<String>::new());
    assert_eq!(fx.found_movies("ronin").await, ["Ronin"]);
    fx.contents
        .record(vec![(item, Decided::Unmatched(None))])
        .await
        .unwrap();
    assert_eq!(fx.found_movies("ronin").await, Vec::<String>::new());
}

#[tokio::test]
async fn a_search_skips_an_episode_whose_match_has_no_show_id() {
    let fx = Fixture::new().await;
    let (library, _folder) = fx.shows_library().await;
    let Decided::Matched(mut showless) = office_episode(9001, "Pilot") else {
        panic!("office_episode Matches");
    };
    showless.show_ids = ExternalIds::default();
    fx.decide_office_episode(library, 1, Decided::Matched(showless))
        .await;

    let found = fx.contents.search("pilot").await.unwrap();

    assert!(found.movies.is_empty(), "{:?}", found.movies);
    assert!(found.episodes.is_empty(), "{:?}", found.episodes);
}

#[tokio::test]
async fn a_search_hit_for_a_title_with_only_a_made_up_id_carries_that_id() {
    let fx = Fixture::new().await;
    let anidb = ExternalId::new("anidb", "m-1").unwrap();
    let matched = Metadata {
        ids: ExternalIds::from_iter([anidb.clone()]),
        ..tmdb_movie(0, "Akira")
    };
    MatchedMovie::store(&fx.contents, fx.library, "/media/Akira.mkv", matched).await;

    let found = fx.contents.search("akira").await.unwrap().movies;
    let ids: Vec<_> = found.into_iter().map(|movie| movie.id).collect();
    assert_eq!(ids, [anidb]);
}
