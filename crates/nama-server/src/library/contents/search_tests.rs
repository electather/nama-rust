use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};
use tempfile::TempDir;

use super::super::{
    Change, Decided, Found, Item, Metadata,
    fixture::{MatchedMovie, decide, tmdb_movie},
    testing::Fixture,
};
use super::match_bands;
use crate::library::{TitleKind, libraries::Kind};

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

    /// The best 20 of what a search of every kind for `query` finds.
    async fn search(&self, query: &str) -> Vec<Found> {
        self.contents.search(query, None, 20).await.unwrap()
    }

    /// The titles of the Movies a search for `query` finds, best first.
    async fn found_movies(&self, query: &str) -> Vec<String> {
        let found = self.search(query).await;
        (found.into_iter())
            .filter_map(|found| match found {
                Found::Movie(movie) => Some(movie.title),
                Found::Show(_) | Found::Episode(_) => None,
            })
            .collect()
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
    let found: Vec<_> = (fx.search("dune 2021").await.into_iter())
        .map(|found| match found {
            Found::Movie(movie) => movie.release_date,
            Found::Show(_) | Found::Episode(_) => panic!("only Movies are in the Library"),
        })
        .collect();
    assert_eq!(found, [Some("2021-09-15".to_owned())]);
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
        titled_backdrop: None,
        logo: None,
        still: None,
        genres: Vec::new(),
        content_rating: None,
        show_poster: None,
        show_overview: None,
        show_first_air_date: None,
        score: None,
        original_language: None,
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

    let found = fx.search("office").await;

    let [Found::Show(show), Found::Episode(episode)] = &found[..] else {
        panic!("The Office, then one of its Episodes: {found:?}");
    };
    assert_eq!((show.library, show.title.as_str()), (library, "The Office"));
    let episode = (
        &episode.show,
        episode.season,
        episode.number,
        episode.title.as_str(),
    );
    assert_eq!(
        episode,
        (&ExternalId::tmdb(THE_OFFICE), 1, 2, "Office Olympics")
    );
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
async fn a_search_skips_a_show_whose_episodes_have_no_title_id() {
    let fx = Fixture::new().await;
    let (library, _folder) = fx.shows_library().await;
    let Decided::Matched(mut untitled) = office_episode(9001, "Pilot") else {
        panic!("office_episode Matches");
    };
    untitled.ids = ExternalIds::default();
    fx.decide_office_episode(library, 1, Decided::Matched(untitled))
        .await;

    assert!(fx.search("office").await.is_empty());
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

    let found = fx.search("akira").await;
    let [Found::Movie(akira)] = &found[..] else {
        panic!("only Akira: {found:?}");
    };
    assert_eq!(akira.id, anidb);
}

/// The titles `found` lists, in its order.
fn titles(found: &[Found]) -> Vec<&str> {
    (found.iter())
        .map(|found| match found {
            Found::Movie(movie) => movie.title.as_str(),
            Found::Show(show) => show.title.as_str(),
            Found::Episode(episode) => episode.title.as_str(),
        })
        .collect()
}

#[tokio::test]
async fn a_search_lists_exact_titles_then_prefixes_then_all_words_with_episodes_last() {
    let fx = Fixture::new().await;
    let (library, _folder) = fx.shows_library().await;
    fx.decide_office_episode(library, 2, office_episode(9002, "Office Olympics"))
        .await;
    fx.match_movie(1, "The Officers", "2001-01-01").await;
    fx.match_movie(2, "Office Space", "1999-02-19").await;
    fx.match_movie(3, "Office", "2015-05-01").await;

    let found = fx.search("office").await;

    assert_eq!(
        titles(&found),
        [
            "Office",
            "Office Space",
            "The Office",
            "The Officers",
            "Office Olympics"
        ]
    );
}

#[tokio::test]
async fn a_search_counts_a_title_and_its_year_as_exact() {
    let fx = Fixture::new().await;
    fx.match_movie(1, "Dune 2021 Unboxed", "2021-01-01").await;
    fx.match_movie(2, "Dune", "2021-09-15").await;

    let found = fx.search("dune 2021").await;

    assert_eq!(titles(&found), ["Dune", "Dune 2021 Unboxed"]);
}

#[tokio::test]
async fn a_search_of_one_kind_finds_only_that_kind() {
    let fx = Fixture::new().await;
    let (library, _folder) = fx.shows_library().await;
    fx.decide_office_episode(library, 2, office_episode(9002, "Office Olympics"))
        .await;
    fx.match_movie(2, "Office Space", "1999-02-19").await;

    let movies = fx.contents.search("office", Some(TitleKind::Movie), 20);
    let shows = fx.contents.search("office", Some(TitleKind::Show), 20);

    assert_eq!(titles(&movies.await.unwrap()), ["Office Space"]);
    assert_eq!(
        titles(&shows.await.unwrap()),
        ["The Office", "Office Olympics"]
    );
}

#[tokio::test]
async fn a_search_lists_no_more_than_its_limit() {
    let fx = Fixture::new().await;
    for (tmdb, title) in [(1, "Alien"), (2, "Aliens"), (3, "Alien 3")] {
        fx.match_movie(tmdb, title, "1979-05-25").await;
    }

    let found = fx.contents.search("alien", None, 2).await.unwrap();

    assert_eq!(titles(&found)[..1], ["Alien"], "the best first");
    assert_eq!(found.len(), 2);
}

#[tokio::test]
async fn a_search_ending_in_a_year_ranks_titles_starting_with_its_words_by_that_year() {
    let fx = Fixture::new().await;
    fx.match_movie(1, "Dune 2021 Unboxed", "2019-01-01").await;
    fx.match_movie(2, "Dune Messiah", "2021-09-15").await;

    let found = fx.search("dune 2021").await;

    assert_eq!(titles(&found), ["Dune Messiah", "Dune 2021 Unboxed"]);
}

#[test]
fn match_bands_put_an_exact_title_then_a_prefix_then_all_words_then_word_starts() {
    let bands = |title| match_bands("alien", title, None);

    assert!(bands("Alien") < bands("Alien: Romulus"));
    assert!(bands("Alien: Romulus") < bands("Planet of the Alien"));
    assert!(bands("Planet of the Alien") < bands("Planet of Aliens"));
}

#[test]
fn match_bands_put_a_title_of_the_year_asked_for_first() {
    let bands = |year| match_bands("dune 2021", "Dune: Messiah", Some(year));

    assert!(bands("2021") < bands("1984"));
}

#[tokio::test]
async fn a_search_by_a_show_title_skips_a_movie_whose_match_has_one() {
    let fx = Fixture::new().await;
    let matched = Metadata {
        show_title: Some("Alien Saga".to_owned()),
        ..tmdb_movie(348, "Alien")
    };
    MatchedMovie::store(&fx.contents, fx.library, "/media/348.mkv", matched).await;

    assert!(fx.search("saga").await.is_empty());
}
