use std::{sync::Arc, time::Duration};

use nama_plugin_api::metadata::{ExternalId, ExternalIds, IMDB};
use tempfile::TempDir;

use crate::{
    db::{self, Database},
    library::{
        contents::{
            Change, Contents, Decided, Item, Metadata,
            fixture::{MatchedMovie, decide, movies_library, tmdb_movie},
            primary_id,
            testing::TitleRow,
        },
        libraries::{Kind, Libraries},
    },
};

/// A Movies Library and a Shows Library.
struct Fixture {
    contents: Contents,
    libraries: Libraries,
    movies: i64,
    shows: i64,
    _dir: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(dir.path()).await.unwrap());
        let (movies_folder, shows_folder) = (dir.path().join("movies"), dir.path().join("shows"));
        std::fs::create_dir(&movies_folder).unwrap();
        std::fs::create_dir(&shows_folder).unwrap();
        let movies = movies_library(&db, &movies_folder).await;
        let libraries = Libraries::new(Arc::clone(&db));
        let shows_folder = shows_folder.canonicalize().unwrap();
        let shows_folder = shows_folder.to_str().unwrap().to_owned();
        let shows = (libraries
            .create("Shows", Kind::Shows, vec![shows_folder])
            .await)
            .unwrap()
            .id;
        Self {
            contents: Contents::new(db),
            libraries,
            movies,
            shows,
            _dir: dir,
        }
    }

    /// Stores a Movie file at `path` and Matches it to a Movie with `ids`, returning its Item.
    async fn movie(&self, path: &str, ids: &[ExternalId]) -> i64 {
        let matched = Metadata {
            ids: ids.iter().cloned().collect(),
            ..tmdb_movie(0, "A Movie")
        };
        (MatchedMovie::store(&self.contents, self.movies, path, matched).await).item
    }

    /// Stores an Episode file at `path`, Season and Episode `numbers`, and Matches it to an
    /// Episode with `ids` of a Show with `show_ids`, returning its Item.
    async fn episode(
        &self,
        path: &str,
        numbers: (i64, i64),
        ids: &[ExternalId],
        show_ids: &[ExternalId],
    ) -> i64 {
        let (season, episode) = numbers;
        let item = Item::Episode {
            show: "A Show".to_owned(),
            year: None,
            hint: None,
            season,
            episode,
        };
        self.store(self.shows, path, item).await;
        let matched = Metadata {
            ids: ids.iter().cloned().collect(),
            show_ids: show_ids.iter().cloned().collect(),
            ..tmdb_movie(0, "An Episode")
        };
        let decided = vec![Decided::Matched(Box::new(matched))];
        decide(&self.contents, self.shows, path, decided).await.1[0]
    }

    /// Stores a file at `path` in `library` holding `item`, which Matching hasn't decided yet.
    async fn store(&self, library: i64, path: &str, item: Item) {
        let change = Change::Store {
            path: path.to_owned(),
            size: 10,
            modified_ns: 0,
            items: vec![item],
        };
        self.contents.apply(library, vec![change]).await.unwrap();
    }

    async fn order_titles_by(&self, library: i64, namespaces: &[&str]) {
        let order = namespaces.iter().map(ToString::to_string).collect();
        (self.libraries.set_title_sources(library, order).await).unwrap();
    }

    /// Each row's `(listed_id, own_id, item, nth)`.
    async fn places(&self, library: i64) -> Vec<(String, String, i64, i64)> {
        let rows = self.contents.library_titles(library).await;
        (rows.into_iter())
            .map(|row| (row.listed_id, row.own_id, row.item, row.nth))
            .collect()
    }

    async fn listed_ids(&self, library: i64) -> Vec<String> {
        let rows = self.contents.library_titles(library).await;
        rows.into_iter().map(|row| row.listed_id).collect()
    }
}

fn id(namespace: &str, value: &str) -> ExternalId {
    ExternalId::new(namespace, value).unwrap()
}

fn a_movie(title: &str) -> Item {
    Item::Movie {
        title: title.to_owned(),
        year: None,
        hint: None,
    }
}

#[tokio::test]
async fn the_preferred_id_follows_the_librarys_title_sources_and_moves_when_they_are_reordered() {
    let fx = Fixture::new().await;
    fx.movie(
        "/m/Alien.mkv",
        &[ExternalId::tmdb(348), ExternalId::tvdb(1)],
    )
    .await;

    let before = fx.listed_ids(fx.movies).await;
    fx.order_titles_by(fx.movies, &["tvdb", "tmdb"]).await;
    let after = fx.listed_ids(fx.movies).await;

    assert_eq!(before, ["tmdb-348"]);
    assert_eq!(after, ["tvdb-1"]);
}

#[tokio::test]
async fn a_title_with_no_ordered_id_takes_its_first_by_namespace_and_a_made_up_one_can_be_ordered()
{
    let fx = Fixture::new().await;
    let ids = [id(IMDB, "tt1"), id("anidb", "5"), id("mal", "7")];
    fx.movie("/m/Akira.mkv", &ids).await;

    let unordered = fx.listed_ids(fx.movies).await;
    fx.order_titles_by(fx.movies, &["mal", "tmdb"]).await;
    let ordered = fx.listed_ids(fx.movies).await;

    assert_eq!(unordered, ["anidb-5"]);
    assert_eq!(ordered, ["mal-7"]);
}

#[tokio::test]
async fn two_movie_files_matched_to_one_id_are_one_title_placed_by_path() {
    let fx = Fixture::new().await;
    let later = fx.movie("/m/b.mkv", &[ExternalId::tmdb(348)]).await;
    let first = fx.movie("/m/a.mkv", &[ExternalId::tmdb(348)]).await;

    let places = fx.places(fx.movies).await;

    let alien = || "tmdb-348".to_owned();
    assert_eq!(
        places,
        [(alien(), alien(), first, 1), (alien(), alien(), later, 2)]
    );
}

#[tokio::test]
async fn episodes_are_one_show_placed_by_season_then_episode_then_path() {
    let fx = Fixture::new().await;
    let show = [ExternalId::tmdb(100)];
    let (s1e1, s1e2, s2e1) = (
        ExternalId::tmdb(11),
        ExternalId::tmdb(12),
        ExternalId::tmdb(21),
    );
    let third = fx.episode("/s/a S02E01.mkv", (2, 1), &[s2e1], &show).await;
    let second = fx.episode("/s/a S01E02.mkv", (1, 2), &[s1e2], &show).await;
    let first_later_path = fx
        .episode(
            "/s/z S01E01.mkv",
            (1, 1),
            std::slice::from_ref(&s1e1),
            &show,
        )
        .await;
    let first = fx.episode("/s/y S01E01.mkv", (1, 1), &[s1e1], &show).await;

    let places = fx.places(fx.shows).await;

    let place = |own: &str, item, nth| ("tmdb-100".to_owned(), own.to_owned(), item, nth);
    assert_eq!(
        places,
        [
            place("tmdb-11", first, 1),
            place("tmdb-11", first_later_path, 2),
            place("tmdb-12", second, 3),
            place("tmdb-21", third, 4),
        ]
    );
}

#[tokio::test]
async fn versions_whose_matches_hold_different_ids_are_one_title_by_preferred_id() {
    let fx = Fixture::new().await;
    fx.movie("/m/a.mkv", &[ExternalId::tmdb(348), id(IMDB, "tt0078748")])
        .await;
    fx.movie("/m/b.mkv", &[ExternalId::tmdb(348), ExternalId::tvdb(9)])
        .await;

    let rows = fx.contents.library_titles(fx.movies).await;

    let titles: Vec<_> = (rows.iter())
        .map(|row| (row.listed_id.as_str(), row.own_id.as_str(), row.nth))
        .collect();
    assert_eq!(
        titles,
        [("tmdb-348", "tmdb-348", 1), ("tmdb-348", "tmdb-348", 2)]
    );
}

#[tokio::test]
async fn unmatched_and_not_yet_matched_items_are_left_out() {
    let fx = Fixture::new().await;
    fx.store(fx.movies, "/m/Undecided.mkv", a_movie("Undecided"))
        .await;
    fx.store(fx.movies, "/m/Unmatched.mkv", a_movie("Unmatched"))
        .await;
    let unmatched = vec![Decided::Unmatched(None)];
    decide(&fx.contents, fx.movies, "/m/Unmatched.mkv", unmatched).await;
    fx.movie("/m/Alien.mkv", &[ExternalId::tmdb(348)]).await;

    let listed = fx.listed_ids(fx.movies).await;

    assert_eq!(listed, ["tmdb-348"]);
}

#[tokio::test]
async fn an_episode_without_a_show_id_and_a_match_without_a_title_id_are_left_out() {
    let fx = Fixture::new().await;
    let show = [ExternalId::tmdb(100)];
    fx.episode("/s/E01.mkv", (1, 1), &[ExternalId::tmdb(11)], &[])
        .await;
    fx.episode("/s/E02.mkv", (1, 2), &[], &show).await;
    let listed = fx
        .episode("/s/E03.mkv", (1, 3), &[ExternalId::tmdb(13)], &show)
        .await;
    fx.movie("/m/Untitled.mkv", &[]).await;

    let shows = fx.places(fx.shows).await;
    let movies = fx.places(fx.movies).await;

    let own = |id: &str| id.to_owned();
    assert_eq!(shows, [(own("tmdb-100"), own("tmdb-13"), listed, 1)]);
    assert!(movies.is_empty(), "{movies:?}");
}

#[tokio::test]
async fn a_movie_was_added_when_its_first_version_was_and_a_show_when_its_newest_episode_was() {
    let fx = Fixture::new().await;
    let (alien, aliens) = ([ExternalId::tmdb(348)], [ExternalId::tmdb(679)]);
    let (dark, lost) = ([ExternalId::tmdb(100)], [ExternalId::tmdb(200)]);
    // Each title gets another title's Item between two of its own, so which side of that one it
    // lands on says which of its own Items its added time is.
    fx.movie("/m/Alien.mkv", &alien).await;
    fx.episode("/s/Dark E01.mkv", (1, 1), &[ExternalId::tmdb(11)], &dark)
        .await;
    db::advance_clock(Duration::from_secs(1));
    fx.movie("/m/Aliens.mkv", &aliens).await;
    fx.episode("/s/Lost E01.mkv", (1, 1), &[ExternalId::tmdb(21)], &lost)
        .await;
    db::advance_clock(Duration::from_secs(1));
    fx.movie("/m/Alien 4K.mkv", &alien).await;
    fx.episode("/s/Dark E02.mkv", (1, 2), &[ExternalId::tmdb(12)], &dark)
        .await;

    let movies = fx.contents.library_titles(fx.movies).await;
    let shows = fx.contents.library_titles(fx.shows).await;

    let added = |rows: &[TitleRow], title: &str| -> Vec<i64> {
        (rows.iter().filter(|row| row.listed_id == title))
            .map(|row| row.added_at)
            .collect()
    };
    let [alien_first, alien_later] = added(&movies, "tmdb-348")[..] else {
        panic!("Alien has two Versions: {movies:?}");
    };
    let [dark_first, dark_later] = added(&shows, "tmdb-100")[..] else {
        panic!("Dark has two Episodes: {shows:?}");
    };
    assert_eq!(alien_first, alien_later);
    assert!(alien_first < added(&movies, "tmdb-679")[0]);
    assert_eq!(dark_first, dark_later);
    assert!(dark_first > added(&shows, "tmdb-200")[0]);
}

#[tokio::test]
async fn a_titles_item_is_its_lowest_item_id() {
    let fx = Fixture::new().await;
    let lowest = fx.movie("/m/b.mkv", &[ExternalId::tmdb(348)]).await;
    fx.movie("/m/a.mkv", &[ExternalId::tmdb(348)]).await;

    let rows = fx.contents.library_titles(fx.movies).await;

    let title_items: Vec<_> = rows.iter().map(|row| row.title_item).collect();
    assert_eq!(title_items, [lowest, lowest]);
}

#[tokio::test]
async fn primary_id_agrees_with_sql() {
    let fx = Fixture::new().await;
    // `ani` is a prefix of `anidb`, which a pick matching namespaces as unpadded text would mix up.
    let id_sets: [&[&str]; 6] = [
        &["tmdb", "imdb", "tvdb"],
        &["imdb", "tvdb"],
        &["anidb"],
        &["ani", "anidb"],
        &["anidb", "mal"],
        &["tvdb", "anidb"],
    ];
    let orders: [&[&str]; 5] = [
        &["tmdb"],
        &["tvdb", "tmdb"],
        &["anidb"],
        &["mal", "anidb"],
        &["imdb", "ani", "tvdb"],
    ];
    let mut movies = Vec::new();
    for (n, namespaces) in (1..).zip(id_sets) {
        // Each set's values differ from every other's, so no two Movies are one title.
        let ids: ExternalIds = (namespaces.iter())
            .map(|namespace| id(namespace, &n.to_string()))
            .collect();
        let ids_vec: Vec<_> = ids.iter().collect();
        let item = fx.movie(&format!("/m/{n}.mkv"), &ids_vec).await;
        movies.push((item, ids));
    }

    for order in orders {
        fx.order_titles_by(fx.movies, order).await;
        let rows = fx.contents.library_titles(fx.movies).await;
        for (item, ids) in &movies {
            let picked = rows
                .iter()
                .find(|row| row.item == *item)
                .map(|row| &row.own_id);
            let expected = primary_id(ids, order).map(|id| id.to_string());
            assert_eq!(picked, expected.as_ref(), "{ids:?} in {order:?}");
        }
    }
}
