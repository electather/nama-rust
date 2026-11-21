use std::time::Duration;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use nama_proto::nama::v1::{PageRequest, PageResponse};
use tempfile::TempDir;

use super::{Listing, TitleSort, Watched};
use crate::{
    db,
    library::{
        contents::{
            Change, Decided, Item, Metadata,
            fixture::{decide, tmdb_movie},
            testing::{Fixture, movie},
        },
        libraries::Kind,
    },
    page::Page,
};

const BY_TITLE: Listing = Listing {
    genres: Vec::new(),
    min_year: None,
    max_year: None,
    watched: None,
    sort: TitleSort::Title,
    descending: false,
};

const NEWEST_FIRST: Listing = Listing {
    genres: Vec::new(),
    min_year: None,
    max_year: None,
    watched: None,
    sort: TitleSort::Added,
    descending: true,
};

/// Stores a file at `path` holding one Movie and Matches it as `metadata` says.
async fn store_movie(fx: &Fixture, path: &str, metadata: Metadata) {
    fx.store(path, 10, movie(&metadata.title, 2000)).await;
    let item = fx.item(path).await;
    fx.contents
        .record(vec![(item, Decided::Matched(Box::new(metadata)))])
        .await
        .unwrap();
}

/// A Shows Library beside the fixture's Movies Library, and the folder it is on.
struct Shows {
    library: i64,
    _folder: TempDir,
}

impl Shows {
    async fn new(fx: &Fixture) -> Self {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let root = root.to_str().unwrap().to_owned();
        let library = (fx.libraries.create("Shows", Kind::Shows, vec![root]).await)
            .unwrap()
            .id;
        Self {
            library,
            _folder: folder,
        }
    }

    /// Stores Episode `number` of the Show called `title`, and Matches it to Episode `episode` of
    /// TMDB's Show `show`.
    async fn store(&self, fx: &Fixture, (title, show): (&str, i64), number: i64, episode: i64) {
        let path = format!("/media/{title}/{title} - S01E{number:02}.mkv");
        let change = Change::Store {
            edition: None,
            path: path.clone(),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Episode {
                show: title.to_owned(),
                year: None,
                hint: None,
                season: 1,
                episode: number,
            }],
        };
        fx.contents.apply(self.library, vec![change]).await.unwrap();
        let metadata = Metadata {
            ids: ExternalIds::from_iter([ExternalId::tmdb(episode)]),
            show_ids: ExternalIds::from_iter([ExternalId::tmdb(show)]),
            show_title: Some(title.to_owned()),
            ..tmdb_movie(0, "An Episode")
        };
        let matched = vec![Decided::Matched(Box::new(metadata))];
        decide(&fx.contents, self.library, &path, matched).await;
    }
}

/// Stores one Movie per `(tmdb, title)` of `movies`, each a second after the last.
async fn store_movies(fx: &Fixture, movies: &[(i64, &str)]) {
    for &(tmdb, title) in movies {
        store_movie(fx, &format!("/media/{title}.mkv"), tmdb_movie(tmdb, title)).await;
        db::advance_clock(Duration::from_secs(1));
    }
}

async fn page_of(
    fx: &Fixture,
    library: i64,
    listing: &Listing,
    started: &[(String, bool)],
    asked: &PageRequest,
) -> (Vec<i64>, PageResponse) {
    let page = Page::read(asked, b"test").unwrap();
    let (ids, paged) = (fx.contents)
        .title_page(library, listing, started, &page)
        .await
        .unwrap();
    let tmdb = ids.iter().map(|id| id.value().parse().unwrap()).collect();
    (tmdb, paged)
}

/// The TMDB ids of the first page of Movies `listing` lists, for a User who started none.
async fn movies(fx: &Fixture, listing: &Listing) -> Vec<i64> {
    page_of(fx, fx.library, listing, &[], &PageRequest::default())
        .await
        .0
}

#[tokio::test]
async fn titles_sort_without_case_accents_or_a_leading_article() {
    let fx = Fixture::new().await;
    let titles = [
        (1, "Memento"),
        (2, "The Matrix"),
        (3, "Matilda"),
        (4, "éclair"),
    ];
    store_movies(&fx, &titles).await;
    store_movies(&fx, &[(5, "Eclair"), (6, "Fargo")]).await;

    assert_eq!(movies(&fx, &BY_TITLE).await, [4, 5, 6, 3, 2, 1]);
}

#[tokio::test]
async fn a_library_of_250_movies_lists_50_on_the_first_page_and_counts_250() {
    let fx = Fixture::new().await;
    for tmdb in 1..=250 {
        let title = format!("Movie {tmdb:03}");
        store_movie(
            &fx,
            &format!("/media/{title}.mkv"),
            tmdb_movie(tmdb, &title),
        )
        .await;
    }

    let (first, paged) = page_of(&fx, fx.library, &BY_TITLE, &[], &PageRequest::default()).await;

    assert_eq!(first, (1..=50).collect::<Vec<_>>());
    assert_eq!(paged.total_size, Some(250));
    assert_ne!(paged.next_token, "");
}

#[tokio::test]
async fn pages_follow_on_from_their_token_and_only_the_first_counts() {
    let fx = Fixture::new().await;
    let titles = [(1, "A1"), (2, "B2"), (3, "C3"), (4, "D4"), (5, "E5")];
    store_movies(&fx, &titles).await;
    let asked = |token: &str| PageRequest {
        size: 2,
        token: token.to_owned(),
        ..Default::default()
    };

    let (first, paged) = page_of(&fx, fx.library, &BY_TITLE, &[], &asked("")).await;
    let (second, later) = page_of(&fx, fx.library, &BY_TITLE, &[], &asked(&paged.next_token)).await;
    let (last, end) = page_of(&fx, fx.library, &BY_TITLE, &[], &asked(&later.next_token)).await;

    assert_eq!((first, paged.total_size), (vec![1, 2], Some(5)));
    assert_eq!((second, later.total_size), (vec![3, 4], None));
    assert_eq!((last, end.next_token), (vec![5], String::new()));
}

#[tokio::test]
async fn a_movie_added_later_comes_first_and_a_second_version_does_not_move_it() {
    let fx = Fixture::new().await;
    store_movies(&fx, &[(1, "Old"), (2, "New")]).await;
    store_movie(&fx, "/media/Old (copy).mkv", tmdb_movie(1, "Old")).await;

    assert_eq!(movies(&fx, &NEWEST_FIRST).await, [2, 1]);
}

#[tokio::test]
async fn a_new_episode_moves_its_show_up() {
    let fx = Fixture::new().await;
    let shows = Shows::new(&fx).await;
    shows.store(&fx, ("Dark", 10), 1, 101).await;
    db::advance_clock(Duration::from_secs(1));
    shows.store(&fx, ("Lost", 20), 1, 201).await;
    db::advance_clock(Duration::from_secs(1));
    shows.store(&fx, ("Dark", 10), 2, 102).await;

    let (listed, _) = page_of(
        &fx,
        shows.library,
        &NEWEST_FIRST,
        &[],
        &PageRequest::default(),
    )
    .await;

    assert_eq!(listed, [10, 20]);
}

#[tokio::test]
async fn a_genre_or_year_filter_keeps_only_titles_that_pass_and_counts_them() {
    let fx = Fixture::new().await;
    let dated = |tmdb, title: &str, date: &str, genre: &str| Metadata {
        release_date: Some(date.to_owned()),
        genres: vec![genre.to_owned()],
        ..tmdb_movie(tmdb, title)
    };
    store_movie(&fx, "/1.mkv", dated(1, "Alien", "1979-05-25", "Horror")).await;
    store_movie(&fx, "/2.mkv", dated(2, "Heat", "1995-12-15", "Crime")).await;
    store_movie(&fx, "/3.mkv", dated(3, "Scream", "1996-12-20", "Horror")).await;
    let horror = Listing {
        genres: vec!["Horror".to_owned(), "Western".to_owned()],
        ..BY_TITLE
    };
    let nineties = Listing {
        min_year: Some(1990),
        max_year: Some(1995),
        ..BY_TITLE
    };

    let (found, paged) = page_of(&fx, fx.library, &horror, &[], &PageRequest::default()).await;

    assert_eq!((found, paged.total_size), (vec![1, 3], Some(2)));
    assert_eq!(movies(&fx, &nineties).await, [2]);
    assert_eq!(
        fx.contents.genres(fx.library).await.unwrap(),
        ["Crime", "Horror"]
    );
}

#[tokio::test]
async fn the_genres_are_those_of_the_version_each_title_shows() {
    let fx = Fixture::new().await;
    let in_genre = |genre: &str| Metadata {
        genres: vec![genre.to_owned()],
        ..tmdb_movie(1, "Alien")
    };
    store_movie(&fx, "/a.mkv", in_genre("Horror")).await;
    store_movie(&fx, "/b.mkv", in_genre("Western")).await;

    let genres = fx.contents.genres(fx.library).await.unwrap();

    assert_eq!(genres, ["Horror"]);
}

#[tokio::test]
async fn the_watched_filter_reads_the_users_watch_states_rolling_a_show_up() {
    let fx = Fixture::new().await;
    store_movies(&fx, &[(1, "Alien"), (2, "Heat"), (3, "Scream")]).await;
    let shows = Shows::new(&fx).await;
    shows.store(&fx, ("Dark", 10), 1, 101).await;
    shows.store(&fx, ("Dark", 10), 2, 102).await;
    shows.store(&fx, ("Lost", 20), 1, 201).await;
    let started = [
        ("tmdb-1".to_owned(), true),
        ("tmdb-2".to_owned(), false),
        ("tmdb-101".to_owned(), true),
        ("tmdb-201".to_owned(), true),
    ];
    let kept = async |library, watched: Watched| {
        let keeping = Listing {
            watched: Some(watched),
            ..BY_TITLE
        };
        page_of(&fx, library, &keeping, &started, &PageRequest::default())
            .await
            .0
    };

    assert_eq!(kept(fx.library, Watched::NotStarted).await, [3]);
    assert_eq!(kept(fx.library, Watched::Started).await, [2]);
    assert_eq!(kept(fx.library, Watched::Played).await, [1]);
    assert_eq!(kept(shows.library, Watched::Started).await, [10]);
    assert_eq!(kept(shows.library, Watched::Played).await, [20]);
}
