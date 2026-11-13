//! What a Library holds: the video files a scan stored, the Items their names say they hold, what
//! Matching decided about each Item, and the Matches an admin pinned.
//!
//! [`Contents`] owns the tables (see the crate's `AGENTS.md`); each method is one read or one write.
//! A write that stores Matches also makes the Matched Movies' and Seasons' Requests Available, or
//! Partially Available (ADR 0012).

use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use crate::db::Database;

mod decisions;
mod files;
#[cfg(test)]
pub(crate) mod fixture;
mod ids;
mod items;
mod matched;
mod pins;
#[cfg(test)]
mod testing;
#[cfg(test)]
pub(crate) use testing::{File, FileItem, Recorded};

use decisions::insert_decisions;
pub(crate) use decisions::{Decided, Decisions, Metadata, TiedTitle, Unmatched, UnmatchedReason};
pub(crate) use files::{Change, Stored};
pub(crate) use ids::parse_hint;
use ids::primary_id;
pub(crate) use items::{Item, Name};
use items::{ItemKind, ItemRow};
pub(crate) use matched::{
    Episode, MatchedTo, Movie, NewestBackdrop, Season, Show, Version, library_with_movie,
    matched_episodes, show_poster,
};
pub(crate) use pins::{Named, Pin};

/// Reads and writes the contents of Libraries.
#[derive(Debug, Clone)]
pub(crate) struct Contents {
    db: Arc<Database>,
}

/// What [`Contents::search`] found in every Library, each kind best fit first, and each Movie,
/// Show or Episode once per Library that has it.
#[derive(Debug, Default)]
pub(crate) struct Found {
    pub(crate) movies: Vec<FoundMovie>,
    /// Found by their own titles.
    pub(crate) shows: Vec<FoundShow>,
    /// Found by their own titles, not their Shows'.
    pub(crate) episodes: Vec<FoundEpisode>,
}

/// A Movie a search found, with the metadata the Match of its best-fitting Item stores.
#[derive(Debug)]
pub(crate) struct FoundMovie {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The Plugin whose metadata Provider made the Match, which the poster's path is for.
    pub(crate) plugin: String,
    pub(crate) title: String,
    pub(crate) release_date: Option<String>,
    pub(crate) poster: Option<String>,
}

/// A Show a search found, with the metadata the Match of its best-fitting Episode stores.
#[derive(Debug)]
pub(crate) struct FoundShow {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The Plugin whose metadata Provider made the Match, which the poster's path is for.
    pub(crate) plugin: String,
    pub(crate) title: String,
    /// As [`Show::poster`] picks it, from the Seasons of the Show's Episodes the search found.
    pub(crate) poster: Option<String>,
}

/// An Episode a search found, with what browsing needs to find it: its Show and Season.
#[derive(Debug)]
pub(crate) struct FoundEpisode {
    pub(crate) library: i64,
    /// As browsing identifies it: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// Its Show's id, as browsing identifies it.
    pub(crate) show: ExternalId,
    pub(crate) show_title: String,
    pub(crate) season: i64,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
}

impl Contents {
    /// The contents of the Libraries in `db`.
    #[must_use]
    pub(crate) const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// The Matched Movies, Shows and Episodes of every Library whose titles have each of the
    /// words of `query`, ignoring case, accents and the marks that join words. A year after
    /// other words keeps only those of that year, or with it in their title.
    pub(crate) async fn search(&self, query: &str) -> Result<Found, sqlx::Error> {
        let mut found = Found::default();
        let Some(words) = SearchWords::of(query) else {
            return Ok(found);
        };
        for row in self.search_rows(&words.in_column("title")).await? {
            let Some(id) = primary_id(row.ids()) else {
                continue;
            };
            match (
                row.kind,
                row.season,
                row.episode,
                primary_id(row.show_ids()),
            ) {
                (ItemKind::Episode, Some(season), Some(number), Some(show)) => {
                    found.episodes.push(FoundEpisode {
                        library: row.library,
                        id,
                        show,
                        show_title: row.show_title.unwrap_or_default(),
                        season,
                        number,
                        title: row.title,
                    });
                }
                // Skipped as `Contents::shows` skips it, so browsing can open what's found.
                (ItemKind::Episode, ..) => {}
                (ItemKind::Movie, ..) => found.movies.push(FoundMovie {
                    library: row.library,
                    id,
                    plugin: row.plugin,
                    title: row.title,
                    release_date: row.release_date,
                    poster: row.poster,
                }),
            }
        }
        found.shows = found_shows(self.search_rows(&words.in_column("show_title")).await?);
        // A Movie or Episode has a row per Item; the first fits best.
        let mut seen = HashSet::new();
        found
            .movies
            .retain(|movie| seen.insert((movie.library, movie.id.clone())));
        seen.clear();
        found
            .episodes
            .retain(|episode| seen.insert((episode.library, episode.id.clone())));
        Ok(found)
    }

    /// The Matched Items whose `title_search` row fits the FTS5 query `fts_query`, best fit first.
    async fn search_rows(&self, fts_query: &str) -> Result<Vec<SearchRow>, sqlx::Error> {
        sqlx::query_as!(
            SearchRow,
            r#"SELECT library_files.library_id AS library,
                      library_items.kind AS "kind: ItemKind", library_items.season,
                      library_items.episode, item_matches.plugin AS "plugin!",
                      item_matches.tmdb_id, item_matches.imdb_id, item_matches.tvdb_id,
                      item_matches.show_tmdb_id, item_matches.show_imdb_id,
                      item_matches.show_tvdb_id, item_matches.title AS "title!",
                      item_matches.show_title, item_matches.release_date, item_matches.poster
               FROM title_search
               JOIN item_matches ON item_matches.item_id = title_search.rowid
               JOIN library_items ON library_items.id = item_matches.item_id
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE title_search MATCH ?
               ORDER BY title_search.rank"#,
            fts_query
        )
        .fetch_all(self.db.reader())
        .await
    }
}

/// An Item a search found, as [`Contents::search_rows`] reads it.
struct SearchRow {
    library: i64,
    kind: ItemKind,
    /// Set for an Episode.
    season: Option<i64>,
    /// Set for an Episode.
    episode: Option<i64>,
    plugin: String,
    tmdb_id: Option<i64>,
    imdb_id: Option<String>,
    tvdb_id: Option<i64>,
    show_tmdb_id: Option<i64>,
    show_imdb_id: Option<String>,
    show_tvdb_id: Option<i64>,
    title: String,
    show_title: Option<String>,
    release_date: Option<String>,
    poster: Option<String>,
}

impl SearchRow {
    fn ids(&self) -> ExternalIds {
        ExternalIds {
            tmdb: self.tmdb_id,
            imdb: self.imdb_id.clone(),
            tvdb: self.tvdb_id,
        }
    }

    fn show_ids(&self) -> ExternalIds {
        ExternalIds {
            tmdb: self.show_tmdb_id,
            imdb: self.show_imdb_id.clone(),
            tvdb: self.show_tvdb_id,
        }
    }
}

/// The Shows that `rows`, best fit first, are Episodes of: each once per Library, in the place of
/// its first row.
fn found_shows(rows: Vec<SearchRow>) -> Vec<FoundShow> {
    let rows: Vec<(ExternalId, SearchRow)> = (rows.into_iter())
        .filter_map(|row| Some((primary_id(row.show_ids())?, row)))
        .collect();
    let mut seen = HashSet::new();
    let mut shows = Vec::new();
    for (id, row) in &rows {
        let Some(title) = &row.show_title else {
            continue;
        };
        if !seen.insert((row.library, id)) {
            continue;
        }
        // Every Episode of a Show fits its title alike, so its rows hold all its Seasons, unless
        // the search ends in a year: that keeps only the Episodes aired in it.
        // ponytail: a year can pick another Season's Poster than the Library grid's; search the
        // Show's rows again without it if that's ever seen.
        // ponytail: rescans every row per Show found; group rows by Show once searches get slow.
        let seasons = (rows.iter())
            .filter(|(other_id, other)| other.library == row.library && other_id == id)
            .filter_map(|(_, other)| Some((other.season?, other.poster.as_deref())));
        shows.push(FoundShow {
            library: row.library,
            id: id.clone(),
            plugin: row.plugin.clone(),
            title: title.clone(),
            poster: show_poster(seasons).map(str::to_owned),
        });
    }
    shows
}

/// A search query's words, and the year it ends with, if any.
#[derive(Debug, PartialEq, Eq)]
struct SearchWords<'a> {
    words: Vec<&'a str>,
    year: Option<&'a str>,
}

impl<'a> SearchWords<'a> {
    /// `query`'s runs of letters and digits, or `None` if it has none. A last word of four digits
    /// after others is the year.
    fn of(query: &'a str) -> Option<Self> {
        let mut words: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect();
        let year = match words.as_slice() {
            [_, .., last] if last.len() == 4 && last.bytes().all(|b| b.is_ascii_digit()) => {
                words.pop()
            }
            _ => None,
        };
        (!words.is_empty()).then_some(Self { words, year })
    }

    /// The FTS5 query for `title_search` rows whose `column` has a word starting with each word,
    /// so a client can search as the User types, and of the year, if any.
    fn in_column(&self, column: &str) -> String {
        // Every word is letters and digits, so quoting it keeps FTS5 from reading it as `AND`,
        // `NOT` or the like.
        let words: Vec<String> = self
            .words
            .iter()
            .map(|word| format!("\"{word}\"*"))
            .collect();
        let words = format!("{column} : ({})", words.join(" "));
        match self.year {
            Some(year) => format!("{words} AND (year : \"{year}\" OR {column} : \"{year}\")"),
            None => words,
        }
    }
}

#[cfg(test)]
mod tests {
    use nama_plugin_api::{
        PluginId,
        metadata::{ExternalId, ExternalIds},
    };
    use tempfile::TempDir;

    use super::{
        Change, Decided, Item, Metadata,
        fixture::{MatchedMovie, decide, tmdb_movie},
        testing::Fixture,
    };
    use crate::libraries::Kind;

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
            ids: ExternalIds {
                tmdb: Some(tmdb),
                ..ExternalIds::default()
            },
            show_ids: ExternalIds {
                tmdb: Some(THE_OFFICE),
                ..ExternalIds::default()
            },
            title: title.to_owned(),
            original_title: None,
            show_title: Some("The Office".to_owned()),
            overview: None,
            release_date: Some("2005-03-24".to_owned()),
            runtime_minutes: None,
            poster: None,
            backdrop: None,
            still: None,
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
            [(&ExternalId::Tmdb(THE_OFFICE), 1, 2, "Office Olympics")]
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
}
