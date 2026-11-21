//! What the Items of a Library are Matched to, as browsing, playback, Requests and artwork read it.

use std::collections::{BTreeMap, HashSet};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{
    Contents, ImageRef, ItemKind, Stored, ids_from_json, ids_sql, library_titles, parse_hint,
    primary_id_in,
};

/// A Movie as `library_titles` groups it, with the metadata the Match of its first Version
/// stores. Browse hands it out with Versions of its own, which have Tracks.
#[derive(Debug)]
pub(crate) struct Movie<V = Version> {
    /// Its `listed_id` in `library_titles`.
    pub(crate) id: ExternalId,
    /// Every id the Match of its first Version gives it.
    pub(crate) ids: ExternalIds,
    /// The Plugin whose title source made the Match.
    pub(crate) plugin: String,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) overview: Option<String>,
    pub(crate) release_date: Option<String>,
    pub(crate) runtime_minutes: Option<i64>,
    pub(crate) poster: Option<ImageRef>,
    pub(crate) backdrop: Option<ImageRef>,
    /// The Backdrop with its title written on it, in the language artwork is picked in, for
    /// clients that show it without its Title Logo over it.
    pub(crate) titled_backdrop: Option<ImageRef>,
    pub(crate) logo: Option<ImageRef>,
    pub(crate) genres: Vec<String>,
    pub(crate) content_rating: Option<String>,
    /// Its Score from the title source that made the Match, kept once per title.
    pub(crate) score: Option<f64>,
    pub(crate) original_language: Option<String>,
    /// Its `added_at` in `library_titles`, in seconds since the Unix epoch.
    pub(crate) added_at: i64,
    /// The files Matched to it, by path.
    pub(crate) versions: Vec<V>,
}

impl<V> Movie<V> {
    /// The Movie with each of its Versions made into `version(it)`.
    pub(crate) fn map_versions<W>(self, version: impl FnMut(V) -> W) -> Movie<W> {
        Movie {
            id: self.id,
            ids: self.ids,
            plugin: self.plugin,
            title: self.title,
            original_title: self.original_title,
            overview: self.overview,
            release_date: self.release_date,
            runtime_minutes: self.runtime_minutes,
            poster: self.poster,
            backdrop: self.backdrop,
            titled_backdrop: self.titled_backdrop,
            logo: self.logo,
            genres: self.genres,
            content_rating: self.content_rating,
            score: self.score,
            original_language: self.original_language,
            added_at: self.added_at,
            versions: self.versions.into_iter().map(version).collect(),
        }
    }
}

/// A stored file that a Movie or Episode is available in.
#[derive(Debug, Clone)]
pub(crate) struct Version {
    /// Its file as stored now.
    pub(crate) file: Stored,
    pub(crate) path: String,
}

/// What [`Contents::matched_to`] found a Version Matched to.
#[derive(Debug)]
pub(crate) enum MatchedTo {
    /// No stored file has that id.
    NoSuchVersion,
    /// The file holds a Movie or nothing, and no Match: its Item is Unmatched or not Matched yet.
    Nothing,
    /// The Movie's id, as browsing identifies it: see [`primary_id`](super::primary_id).
    Movie(ExternalId),
    /// The file holds Episodes.
    Episodes {
        /// The id of the Show its first Matched Episode is of, as browsing identifies it. `None`
        /// if none is Matched.
        show: Option<ExternalId>,
    },
}

/// What [`Contents::titles_in`] found a Version Matched to, each title by its id as browsing
/// identifies it and every id its Match gives it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TitlesIn {
    /// No stored file has that id.
    NoSuchVersion,
    /// The file holds a Movie or nothing: the Movie, unless its Item isn't Matched.
    Movie(Option<(ExternalId, ExternalIds)>),
    /// Each Episode the file holds, by season and episode number, `None` if its Item isn't
    /// Matched.
    Episodes(Vec<Option<(ExternalId, ExternalIds)>>),
}

/// A Show as `library_titles` groups it, with the metadata the Match of its first Episode
/// stores.
#[derive(Debug, Clone)]
pub(crate) struct Show<V = Version> {
    /// Its `listed_id` in `library_titles`.
    pub(crate) id: ExternalId,
    /// Every Show id that Match gives it.
    pub(crate) ids: ExternalIds,
    pub(crate) title: String,
    /// Its own Poster: see [`Show::poster`] for the one it shows.
    pub(crate) own_poster: Option<ImageRef>,
    /// The [`METADATA_VERSION`](crate::library::versions::METADATA_VERSION) that stored `own_poster`.
    pub(crate) metadata_version: i64,
    pub(crate) backdrop: Option<ImageRef>,
    /// As [`Movie::titled_backdrop`].
    pub(crate) titled_backdrop: Option<ImageRef>,
    pub(crate) logo: Option<ImageRef>,
    pub(crate) overview: Option<String>,
    pub(crate) first_air_date: Option<String>,
    pub(crate) genres: Vec<String>,
    pub(crate) content_rating: Option<String>,
    /// As [`Movie::score`].
    pub(crate) score: Option<f64>,
    pub(crate) original_language: Option<String>,
    /// Its `added_at` in `library_titles`, in seconds since the Unix epoch.
    pub(crate) added_at: i64,
    /// The Seasons the Library has Episodes of, by number.
    pub(crate) seasons: BTreeMap<i64, Season<V>>,
}

/// The metadata version whose Matches first stored a Show's own Poster.
const SHOW_POSTERS_SINCE: i64 = 3;

/// A Show's Poster: `own`, from a Match stored at metadata version `version`. A Match stored
/// before Matches had a Show's own has, until refreshed, the Poster of the lowest-numbered Season
/// after the Specials (Season 0) that has one, or else the Specials'. `seasons` are Season numbers
/// with their Posters, in any order. A Season's Custom Poster never stands for its Show.
pub(crate) fn show_poster<P>(
    own: Option<P>,
    version: i64,
    seasons: impl IntoIterator<Item = (i64, Option<P>)>,
) -> Option<P> {
    if version < SHOW_POSTERS_SINCE {
        first_season_poster(seasons)
    } else {
        own
    }
}

fn first_season_poster<P>(seasons: impl IntoIterator<Item = (i64, Option<P>)>) -> Option<P> {
    (seasons.into_iter())
        .filter_map(|(number, poster)| Some((number, poster?)))
        .min_by_key(|&(number, _)| (number == 0, number))
        .map(|(_, poster)| poster)
}

impl<V> Show<V> {
    /// The Poster it is shown by: see [`show_poster`].
    pub(crate) fn poster(&self) -> Option<&ImageRef> {
        let seasons =
            (self.seasons.iter()).map(|(&number, season)| (number, season.poster.as_ref()));
        show_poster(self.own_poster.as_ref(), self.metadata_version, seasons)
    }

    /// Every Episode of every Season, by season and episode number.
    pub(crate) fn episodes(&self) -> impl Iterator<Item = &Episode<V>> {
        self.seasons.values().flat_map(|season| &season.episodes)
    }
}

/// The Episodes of a Show's Season that some Items of a Library are Matched to.
#[derive(Debug, Clone)]
pub(crate) struct Season<V = Version> {
    /// The poster the Match of its first Episode stores.
    pub(crate) poster: Option<ImageRef>,
    /// By episode number.
    pub(crate) episodes: Vec<Episode<V>>,
}

/// An Episode some Items of a Library are Matched to, with the metadata the Match of its first
/// Version stores.
#[derive(Debug, Clone)]
pub(crate) struct Episode<V = Version> {
    /// Its `own_id` in `library_titles`.
    pub(crate) id: ExternalId,
    /// Every id the Match of its first Version gives it.
    pub(crate) ids: ExternalIds,
    /// Its Season's number.
    pub(crate) season: i64,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
    pub(crate) overview: Option<String>,
    pub(crate) air_date: Option<String>,
    pub(crate) runtime_minutes: Option<i64>,
    /// The Plugin whose title source made the Match, which the Still's path is for.
    pub(crate) plugin: String,
    pub(crate) still: Option<String>,
    /// The files Matched to it, by path. A file holding two Episodes is a Version of each.
    pub(crate) versions: Vec<V>,
}

impl<V> Episode<V> {
    /// The Episode with each of its Versions made into `version(it)`.
    pub(crate) fn map_versions<W>(self, version: impl FnMut(V) -> W) -> Episode<W> {
        Episode {
            id: self.id,
            ids: self.ids,
            season: self.season,
            number: self.number,
            title: self.title,
            overview: self.overview,
            air_date: self.air_date,
            runtime_minutes: self.runtime_minutes,
            plugin: self.plugin,
            still: self.still,
            versions: self.versions.into_iter().map(version).collect(),
        }
    }
}

impl Contents {
    /// What the Version `version` is Matched to, each title by its id as browsing identifies it:
    /// its Movie, or for a file of Episodes only the Show of its first Matched one, by season and
    /// number. [`Contents::titles_in`] gives each Episode, with every id.
    pub(crate) async fn matched_to(&self, version: i64) -> Result<MatchedTo, sqlx::Error> {
        // This query and `titles_in`'s must change together: it reads the same rows, adding only
        // the Show's ids.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_items.kind AS "kind?: ItemKind", "#
                        + $ids
                        + r#"'title') AS "ids?: String", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String", libraries.title_sources
                       FROM library_files
                       JOIN libraries ON libraries.id = library_files.library_id
                       LEFT JOIN library_items ON library_items.file_id = library_files.id
                       LEFT JOIN item_matches ON item_matches.item_id = library_items.id
                                             AND item_matches.state = 'matched'
                       WHERE library_files.id = ?
                       ORDER BY library_items.season, library_items.episode"#,
                    version
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        let Some(first) = rows.first() else {
            return Ok(MatchedTo::NoSuchVersion);
        };
        let holds_episodes = first.kind == Some(ItemKind::Episode);
        let ids: Vec<_> = (rows.into_iter())
            .map(|row| {
                let id = primary_id_in(&ids_from_json(row.ids.as_deref()), &row.title_sources);
                let show =
                    primary_id_in(&ids_from_json(row.show_ids.as_deref()), &row.title_sources);
                (id, show)
            })
            .collect();
        if holds_episodes {
            return Ok(MatchedTo::Episodes {
                show: ids.iter().find_map(|(_, show)| show.clone()),
            });
        }
        Ok((ids.into_iter())
            .find_map(|(id, _)| id)
            .map_or(MatchedTo::Nothing, MatchedTo::Movie))
    }

    /// What the Version `version` is Matched to, as [`Contents::matched_to`] finds it, with every
    /// id of each title, so Watch State is found under any of them.
    pub(crate) async fn titles_in(&self, version: i64) -> Result<TitlesIn, sqlx::Error> {
        // This query and `matched_to`'s must change together: it reads the same rows, leaving out
        // only the Show's ids.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_items.kind AS "kind?: ItemKind", "#
                        + $ids
                        + r#"'title') AS "ids?: String", libraries.title_sources
                       FROM library_files
                       JOIN libraries ON libraries.id = library_files.library_id
                       LEFT JOIN library_items ON library_items.file_id = library_files.id
                       LEFT JOIN item_matches ON item_matches.item_id = library_items.id
                                             AND item_matches.state = 'matched'
                       WHERE library_files.id = ?
                       ORDER BY library_items.season, library_items.episode"#,
                    version
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        let Some(first) = rows.first() else {
            return Ok(TitlesIn::NoSuchVersion);
        };
        let holds_episodes = first.kind == Some(ItemKind::Episode);
        let mut titles = rows.into_iter().map(|row| {
            let ids = ids_from_json(row.ids.as_deref());
            Some((primary_id_in(&ids, &row.title_sources)?, ids))
        });
        Ok(if holds_episodes {
            TitlesIn::Episodes(titles.collect())
        } else {
            TitlesIn::Movie(titles.find_map(|title| title))
        })
    }

    /// What the Version `version` plays, as people name it: its Movie's title, or the Show's of
    /// its first Episode with that Episode, such as "Dark S01E02", and while it isn't Matched the
    /// name its file was found under. `None` if there is no such Version.
    pub(crate) async fn version_title(&self, version: i64) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query!(
            r#"SELECT CASE library_items.kind WHEN 'episode' THEN item_matches.show_title
                                              ELSE item_matches.title END AS "matched?: String",
                      library_items.title AS found, library_items.season, library_items.episode
               FROM library_items
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
                                     AND item_matches.state = 'matched'
               WHERE library_items.file_id = ?
               ORDER BY library_items.season, library_items.episode
               LIMIT 1"#,
            version
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map(|row| {
            let title = row.matched.unwrap_or(row.found);
            match row.season.zip(row.episode) {
                Some((season, episode)) => format!("{title} S{season:02}E{episode:02}"),
                None => title,
            }
        }))
    }

    /// The path of the Version `version` and the file stored there, or `None` unless the file
    /// holds an Item, Matched or not (ADR 0069), so a file whose name didn't parse is never served
    /// or probed.
    pub(crate) async fn version_file(
        &self,
        version: i64,
    ) -> Result<Option<(String, Stored)>, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT path, size, modified_ns, parser_version FROM library_files
             WHERE id = ?
               AND EXISTS (SELECT 1 FROM library_items
                           WHERE library_items.file_id = library_files.id)",
            version
        )
        .fetch_optional(self.db.reader())
        .await?;
        Ok(row.map(|row| {
            let stored = Stored {
                id: version,
                size: row.size,
                modified_ns: row.modified_ns,
                parser_version: row.parser_version,
            };
            (row.path, stored)
        }))
    }

    /// The oldest Library with an Item Matched to the Movie `id`, by any of its ids, if any.
    pub(crate) async fn library_with_movie(
        &self,
        id: &ExternalId,
    ) -> Result<Option<i64>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        library_with_movie(&mut conn, id).await
    }

    /// By title, ignoring case, each as `library_titles` groups it: every Movie, or only those
    /// with one of `only` among any of their Versions' ids, with all their Versions.
    #[expect(
        clippy::too_many_lines,
        reason = "one query's columns, and the Movie each row fills"
    )]
    pub(crate) async fn movies(
        &self,
        library: i64,
        only: Option<&[ExternalId]>,
    ) -> Result<Vec<Movie>, sqlx::Error> {
        let only = listed_ids(only);
        // ponytail: whole-Library read, grouped by `library_titles`.
        macro_rules! select {
            ($ids:literal, ($($titles:tt)+)) => {
                sqlx::query!(
                    r#"WITH titles AS (SELECT * FROM "#
                        + $($titles)+
                        + r#" AS library_titles
                                 WHERE library_id = ?1 AND kind = 'movie')
                       SELECT titles.listed_id AS "listed_id!: String",
                              library_files.id AS "file!", library_files.path,
                              library_files.size, library_files.modified_ns,
                              library_files.parser_version, item_matches.plugin AS "plugin!", "#
                        + $ids
                        + r#"'title') AS "ids?: String", item_matches.title AS "title!",
                              item_matches.original_title, item_matches.overview,
                              item_matches.release_date, item_matches.runtime_minutes,
                              item_matches.poster, item_matches.poster_plugin,
                              item_matches.backdrop, item_matches.backdrop_plugin,
                              item_matches.titled_backdrop, item_matches.titled_backdrop_plugin,
                              item_matches.logo, item_matches.logo_plugin, item_matches.genres,
                              item_matches.content_rating, item_matches.original_language,
                              title_scores.score AS "score?", titles.added_at AS "added_at!: i64"
                       FROM titles
                       JOIN item_matches ON item_matches.item_id = titles.item_id
                       JOIN library_items ON library_items.id = titles.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       -- Keyed as `scores::store` keys it.
                       LEFT JOIN title_scores
                           ON title_scores.plugin = item_matches.plugin
                          AND title_scores.kind = 'movie'
                          AND (title_scores.namespace, title_scores.value) =
                              (SELECT keyed.namespace, keyed.value FROM item_match_ids AS keyed
                               WHERE keyed.item_id = item_matches.item_id
                                 AND keyed.scope = 'title'
                               ORDER BY keyed.namespace LIMIT 1)
                       WHERE ?2 IS NULL
                          OR titles.listed_id IN (
                              SELECT asked.listed_id FROM titles AS asked
                              JOIN item_match_ids AS named
                                ON named.item_id = asked.item_id AND named.scope = 'title'
                              JOIN json_each(?2) AS only
                                ON only.value = named.namespace || '-' || named.value)
                       ORDER BY titles.listed_id, titles.nth"#,
                    library,
                    only
                )
            };
        }
        let rows = ids_sql!(library_titles, select)
            .fetch_all(self.db.reader())
            .await?;
        // By title, then place in it, so each Movie's rows are adjacent, its first Version first.
        let mut movies: Vec<Movie> = Vec::new();
        for row in rows {
            let Some(id) = parse_hint(&row.listed_id) else {
                continue;
            };
            let version = version(
                row.file,
                row.path,
                row.size,
                row.modified_ns,
                row.parser_version,
            );
            match movies.last_mut() {
                Some(movie) if movie.id == id => movie.versions.push(version),
                _ => movies.push(Movie {
                    id,
                    ids: ids_from_json(row.ids.as_deref()),
                    plugin: row.plugin,
                    title: row.title,
                    original_title: row.original_title,
                    overview: row.overview,
                    release_date: row.release_date,
                    runtime_minutes: row.runtime_minutes,
                    poster: ImageRef::stored(row.poster, row.poster_plugin),
                    backdrop: ImageRef::stored(row.backdrop, row.backdrop_plugin),
                    titled_backdrop: ImageRef::stored(
                        row.titled_backdrop,
                        row.titled_backdrop_plugin,
                    ),
                    logo: ImageRef::stored(row.logo, row.logo_plugin),
                    genres: genres(row.genres.as_deref())?,
                    content_rating: row.content_rating,
                    score: row.score,
                    original_language: row.original_language,
                    added_at: row.added_at,
                    versions: vec![version],
                }),
            }
        }
        movies.sort_by_cached_key(|movie| title_order(&movie.title, &movie.id));
        Ok(movies)
    }

    /// By title, ignoring case, each as `library_titles` groups it, each Season's Episodes by
    /// their own preferred id: every Show, or only those with one of `only` among any of their
    /// Episodes' Show ids, with all their Episodes.
    #[expect(
        clippy::too_many_lines,
        reason = "one query's columns, and the Show, Season and Episode each row fills"
    )]
    pub(crate) async fn shows(
        &self,
        library: i64,
        only: Option<&[ExternalId]>,
    ) -> Result<Vec<Show>, sqlx::Error> {
        let only = listed_ids(only);
        // ponytail: whole-Library read, grouped by `library_titles`.
        macro_rules! select {
            ($ids:literal, ($($titles:tt)+)) => {
                sqlx::query!(
                    r#"WITH titles AS (SELECT * FROM "#
                        + $($titles)+
                        + r#" AS library_titles
                                 WHERE library_id = ?1 AND kind = 'episode')
                       SELECT titles.listed_id AS "listed_id!: String",
                              titles.own_id AS "own_id!: String",
                              library_files.id AS "file!", library_files.path,
                              library_files.size, library_files.modified_ns,
                              library_files.parser_version, titles.season AS "season!: i64",
                              titles.episode AS "episode!: i64",
                              item_matches.plugin AS "plugin!", "#
                        + $ids
                        + r#"'title') AS "ids?: String", "#
                        + $ids
                        + r#"'show') AS "show_ids?: String",
                              item_matches.title AS "title!",
                              item_matches.show_title AS "show_title!",
                              overview, release_date, runtime_minutes, poster, poster_plugin,
                              backdrop, backdrop_plugin, titled_backdrop, titled_backdrop_plugin,
                              logo, logo_plugin, still, genres,
                              content_rating, show_poster, show_poster_plugin, show_overview,
                              show_first_air_date, metadata_version, item_matches.original_language,
                              title_scores.score AS "score?", titles.added_at AS "added_at!: i64"
                       FROM titles
                       JOIN item_matches ON item_matches.item_id = titles.item_id
                       JOIN library_items ON library_items.id = titles.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       -- Keyed as `scores::store` keys it.
                       LEFT JOIN title_scores
                           ON title_scores.plugin = item_matches.plugin
                          AND title_scores.kind = 'show'
                          AND (title_scores.namespace, title_scores.value) =
                              (SELECT keyed.namespace, keyed.value FROM item_match_ids AS keyed
                               WHERE keyed.item_id = item_matches.item_id
                                 AND keyed.scope = 'show'
                               ORDER BY keyed.namespace LIMIT 1)
                       WHERE ?2 IS NULL
                          OR titles.listed_id IN (
                              SELECT asked.listed_id FROM titles AS asked
                              JOIN item_match_ids AS named
                                ON named.item_id = asked.item_id AND named.scope = 'show'
                              JOIN json_each(?2) AS only
                                ON only.value = named.namespace || '-' || named.value)
                       ORDER BY titles.listed_id, titles.nth"#,
                    library,
                    only
                )
            };
        }
        let rows = ids_sql!(library_titles, select)
            .fetch_all(self.db.reader())
            .await?;
        // By title, then season, episode and path, so each Show's rows are adjacent, its first
        // Episode first, and within a Season the rows of one Episode are adjacent, by path.
        let mut shows: Vec<Show> = Vec::new();
        for row in rows {
            let (Some(show_id), Some(id)) = (parse_hint(&row.listed_id), parse_hint(&row.own_id))
            else {
                continue;
            };
            if shows.last().is_none_or(|show| show.id != show_id) {
                shows.push(Show {
                    id: show_id,
                    ids: ids_from_json(row.show_ids.as_deref()),
                    title: row.show_title,
                    own_poster: ImageRef::stored(row.show_poster, row.show_poster_plugin),
                    metadata_version: row.metadata_version,
                    backdrop: ImageRef::stored(row.backdrop, row.backdrop_plugin),
                    titled_backdrop: ImageRef::stored(
                        row.titled_backdrop,
                        row.titled_backdrop_plugin,
                    ),
                    logo: ImageRef::stored(row.logo, row.logo_plugin),
                    overview: row.show_overview,
                    first_air_date: row.show_first_air_date,
                    genres: genres(row.genres.as_deref())?,
                    content_rating: row.content_rating,
                    score: row.score,
                    original_language: row.original_language,
                    added_at: row.added_at,
                    seasons: BTreeMap::new(),
                });
            }
            let Some(show) = shows.last_mut() else {
                continue;
            };
            let season = show.seasons.entry(row.season).or_insert_with(|| Season {
                poster: ImageRef::stored(row.poster, row.poster_plugin),
                episodes: Vec::new(),
            });
            let version = version(
                row.file,
                row.path,
                row.size,
                row.modified_ns,
                row.parser_version,
            );
            match season.episodes.last_mut() {
                Some(episode) if episode.id == id => episode.versions.push(version),
                _ => season.episodes.push(Episode {
                    id,
                    ids: ids_from_json(row.ids.as_deref()),
                    season: row.season,
                    number: row.episode,
                    title: row.title,
                    overview: row.overview,
                    air_date: row.release_date,
                    runtime_minutes: row.runtime_minutes,
                    plugin: row.plugin,
                    still: row.still,
                    versions: vec![version],
                }),
            }
        }
        shows.sort_by_cached_key(|show| title_order(&show.title, &show.id));
        Ok(shows)
    }
}

/// The key Movies and Shows sort by: alphabetical whatever the case, then as written so titles
/// differing only in case ("Up", "UP") keep one order, then by `id` so identical ones do too.
fn title_order(title: &str, id: &ExternalId) -> (String, String, ExternalId) {
    (title.to_lowercase(), title.to_owned(), id.clone())
}

impl Contents {
    /// Every id of each of the `named` Movies, or Episodes, of the Library `library`, in the same
    /// order, each found by the id it is named by. One the Library doesn't have keeps only that
    /// id. Reads only the named titles' Matches, however large the Library.
    pub(crate) async fn every_id(
        &self,
        library: i64,
        kind: ItemKind,
        named: &[ExternalId],
    ) -> Result<Vec<ExternalIds>, sqlx::Error> {
        let asked: Vec<[&str; 2]> = (named.iter())
            .map(|id| [id.namespace(), id.value()])
            .collect();
        let asked = serde_json::Value::from(asked).to_string();
        // Where more than one Match has a named id, as Versions of one title or two titles sharing
        // an id, the first row by the ORDER BY below keeps it: by Show title (a Movie has none),
        // season and episode number, then the Match's own title, and only then path, so a title
        // that sorts first keeps the id even where its path sorts last. An Episode without a Show
        // id is left out, as `shows` leaves it out.
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT CAST(asked.key AS INTEGER) AS "position!: i64", "#
                        + $ids
                        + r#"'title') AS "ids?: String"
                       FROM json_each(?2) AS asked
                       JOIN item_match_ids AS named
                           ON named.scope = 'title'
                          AND named.namespace = json_extract(asked.value, '$[0]')
                          AND named.value = json_extract(asked.value, '$[1]')
                       JOIN item_matches ON item_matches.item_id = named.item_id
                       JOIN library_items ON library_items.id = item_matches.item_id
                       JOIN library_files ON library_files.id = library_items.file_id
                       WHERE library_files.library_id = ?1
                         AND library_items.kind = ?3
                         AND item_matches.state = 'matched'
                         AND (library_items.kind = 'movie'
                              OR EXISTS (SELECT 1 FROM item_match_ids AS show
                                         WHERE show.item_id = item_matches.item_id
                                           AND show.scope = 'show'))
                       ORDER BY asked.key, item_matches.show_title, library_items.season,
                                library_items.episode, item_matches.title, library_files.path"#,
                    library,
                    asked,
                    kind
                )
            };
        }
        let rows = ids_sql!(select).fetch_all(self.db.reader()).await?;
        let mut found: Vec<Option<ExternalIds>> = vec![None; named.len()];
        for row in rows {
            let first = usize::try_from(row.position)
                .ok()
                .and_then(|position| found.get_mut(position))
                .filter(|found| found.is_none());
            if let Some(first) = first {
                *first = Some(ids_from_json(row.ids.as_deref()));
            }
        }
        Ok((named.iter().zip(found))
            .map(|(named, found)| found.unwrap_or_else(|| ExternalIds::from_iter([named.clone()])))
            .collect())
    }
}

/// The oldest Library with an Item Matched to the Movie `id`, by any of its ids, if any, read
/// on `conn` so a write can act on the answer before a scan changes it.
pub(crate) async fn library_with_movie(
    conn: &mut SqliteConnection,
    id: &ExternalId,
) -> Result<Option<i64>, sqlx::Error> {
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query_scalar!(
        r#"SELECT MIN(library_files.library_id) AS "library: i64"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_match_ids.scope = 'title'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'movie'"#,
        namespace,
        value
    )
    .fetch_one(&mut *conn)
    .await
}

/// The oldest Library with an Episode Matched to the Show `id`, by any of its ids, if any.
pub(crate) async fn library_with_show(
    conn: &mut SqliteConnection,
    id: &ExternalId,
) -> Result<Option<i64>, sqlx::Error> {
    let (namespace, value) = (id.namespace(), id.value());
    sqlx::query_scalar!(
        r#"SELECT MIN(library_files.library_id) AS "library: i64"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_match_ids.scope = 'show'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'episode'"#,
        namespace,
        value
    )
    .fetch_one(&mut *conn)
    .await
}

/// The numbers of the Episodes of Season `season` of the Show `show`, by any of its ids,
/// Matched in any Library.
pub(crate) async fn matched_episodes(
    conn: &mut SqliteConnection,
    show: &ExternalId,
    season: i64,
) -> Result<HashSet<i64>, sqlx::Error> {
    let (namespace, value) = (show.namespace(), show.value());
    // Matching finds an Episode by the numbers its file's name gives, so they are the Episode's.
    let numbers = sqlx::query_scalar!(
        r#"SELECT library_items.episode AS "episode!"
           FROM item_match_ids
           JOIN item_matches ON item_matches.item_id = item_match_ids.item_id
           JOIN library_items ON library_items.id = item_matches.item_id
           WHERE item_match_ids.scope = 'show'
             AND item_match_ids.namespace = ?
             AND item_match_ids.value = ?
             AND item_matches.state = 'matched'
             AND library_items.kind = 'episode'
             AND library_items.season = ?"#,
        namespace,
        value,
        season
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(numbers.into_iter().collect())
}

/// `ids` as a JSON array of their text forms, as [`Contents::movies`] and [`Contents::shows`]
/// take them.
fn listed_ids(ids: Option<&[ExternalId]>) -> Option<String> {
    ids.map(|ids| serde_json::Value::from_iter(ids.iter().map(ToString::to_string)).to_string())
}

/// The genres in the `genres` column's JSON, `json`: none where it's NULL, as for a Match stored
/// before Matches had them.
pub(super) fn genres(json: Option<&str>) -> Result<Vec<String>, sqlx::Error> {
    json.map_or(Ok(Vec::new()), |json| {
        serde_json::from_str(json).map_err(|err| sqlx::Error::Decode(err.into()))
    })
}

/// The Version of a file that [`Contents::movies`] and [`Contents::shows`] read, its columns
/// in the order both select them.
fn version(id: i64, path: String, size: i64, modified_ns: i64, parser_version: i64) -> Version {
    let file = Stored {
        id,
        size,
        modified_ns,
        parser_version,
    };
    Version { file, path }
}

#[cfg(test)]
#[path = "matched_tests.rs"]
mod tests;
