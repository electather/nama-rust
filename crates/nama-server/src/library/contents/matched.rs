//! What the Items of a Library are Matched to, as browsing, playback, Requests and artwork read it.

use std::collections::{BTreeMap, HashMap, HashSet};

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use super::{Contents, ItemKind, Stored, primary_id};

/// A Library's downloaded Backdrop: see [`Contents::newest_backdrops`].
#[derive(Debug)]
pub(crate) struct NewestBackdrop {
    /// The Plugin whose Provider the path is of.
    pub(crate) plugin: String,
    pub(crate) path: String,
    /// The Movie's id, or for an Episode its Show's, whose own Custom Backdrop goes first.
    pub(crate) title: ExternalId,
}

/// A Movie some Items of a Library are Matched to, with the metadata the Match of its first
/// Version stores. Browse hands it out with Versions of its own, which have Tracks.
#[derive(Debug)]
pub(crate) struct Movie<V = Version> {
    /// Which of the Match's ids identifies the Movie: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The Plugin whose metadata Provider made the Match, which the image paths are for.
    pub(crate) plugin: String,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) overview: Option<String>,
    pub(crate) release_date: Option<String>,
    pub(crate) runtime_minutes: Option<i64>,
    pub(crate) poster: Option<String>,
    pub(crate) backdrop: Option<String>,
    /// The files Matched to it, by path.
    pub(crate) versions: Vec<V>,
}

impl<V> Movie<V> {
    /// The Movie with each of its Versions made into `version(it)`.
    pub(crate) fn map_versions<W>(self, version: impl FnMut(V) -> W) -> Movie<W> {
        Movie {
            id: self.id,
            plugin: self.plugin,
            title: self.title,
            original_title: self.original_title,
            overview: self.overview,
            release_date: self.release_date,
            runtime_minutes: self.runtime_minutes,
            poster: self.poster,
            backdrop: self.backdrop,
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
    /// The Movie's id, as browsing identifies it: see [`primary_id`].
    Movie(ExternalId),
    /// The file holds Episodes.
    Episodes {
        /// The id of the Show its first Matched Episode is of, as browsing identifies it. `None`
        /// if none is Matched.
        show: Option<ExternalId>,
        /// Each Episode the file holds, by season and episode number: its id, as browsing
        /// identifies it, or `None` if its Item isn't Matched.
        episodes: Vec<Option<ExternalId>>,
    },
}

/// A Show some Items of a Library are Matched to Episodes of, with the metadata the Match of its
/// first Episode, by season, episode and path, stores.
#[derive(Debug, Clone)]
pub(crate) struct Show<V = Version> {
    /// Which of its Episodes' Show ids identifies it: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The Show's TVDB id, which Sonarr finds it by, if that Match has one.
    pub(crate) tvdb_id: Option<i64>,
    /// The Plugin whose metadata Provider made the Match, which the image paths are for.
    pub(crate) plugin: String,
    pub(crate) title: String,
    pub(crate) backdrop: Option<String>,
    /// The Seasons the Library has Episodes of, by number.
    pub(crate) seasons: BTreeMap<i64, Season<V>>,
}

/// The poster of the lowest-numbered Season after the Specials (Season 0) that has one, or else
/// the Specials'. A Match stores each Episode's Season poster and none of the Show's own. `seasons` are Season numbers with their posters, in any order.
pub(crate) fn show_poster<'a>(
    seasons: impl IntoIterator<Item = (i64, Option<&'a str>)>,
) -> Option<&'a str> {
    (seasons.into_iter())
        .filter_map(|(number, poster)| Some((number, poster?)))
        .min_by_key(|&(number, _)| (number == 0, number))
        .map(|(_, poster)| poster)
}

impl<V> Show<V> {
    /// Every Episode of every Season, by season and episode number.
    pub(crate) fn episodes(&self) -> impl Iterator<Item = &Episode<V>> {
        self.seasons.values().flat_map(|season| &season.episodes)
    }

    /// The Show with each of its Episodes' Versions made into `version(it)`.
    pub(crate) fn map_versions<W>(self, mut version: impl FnMut(V) -> W) -> Show<W> {
        let seasons = (self.seasons.into_iter())
            .map(|(number, season)| {
                let episodes = (season.episodes.into_iter())
                    .map(|episode| episode.map_versions(&mut version))
                    .collect();
                let season = Season {
                    poster: season.poster,
                    episodes,
                };
                (number, season)
            })
            .collect();
        Show {
            id: self.id,
            tvdb_id: self.tvdb_id,
            plugin: self.plugin,
            title: self.title,
            backdrop: self.backdrop,
            seasons,
        }
    }
}

/// The Episodes of a Show's Season that some Items of a Library are Matched to.
#[derive(Debug, Clone)]
pub(crate) struct Season<V = Version> {
    /// The poster the Match of its first Episode stores.
    pub(crate) poster: Option<String>,
    /// By episode number.
    pub(crate) episodes: Vec<Episode<V>>,
}

/// An Episode some Items of a Library are Matched to, with the metadata the Match of its first
/// Version stores.
#[derive(Debug, Clone)]
pub(crate) struct Episode<V = Version> {
    /// Which of the Match's ids identifies the Episode: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// Its Season's number.
    pub(crate) season: i64,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
    pub(crate) overview: Option<String>,
    pub(crate) air_date: Option<String>,
    pub(crate) runtime_minutes: Option<i64>,
    /// The Plugin whose metadata Provider made the Match, which the Still's path is for.
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
    pub(crate) async fn matched_to(&self, version: i64) -> Result<MatchedTo, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.kind AS "kind?: ItemKind", item_matches.tmdb_id, item_matches.imdb_id,
                      item_matches.tvdb_id, item_matches.show_tmdb_id, item_matches.show_imdb_id,
                      item_matches.show_tvdb_id
               FROM library_files
               LEFT JOIN library_items ON library_items.file_id = library_files.id
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
                                     AND item_matches.state = 'matched'
               WHERE library_files.id = ?
               ORDER BY library_items.season, library_items.episode"#,
            version
        )
        .fetch_all(self.db.reader())
        .await?;
        let Some(first) = rows.first() else {
            return Ok(MatchedTo::NoSuchVersion);
        };
        let holds_episodes = first.kind == Some(ItemKind::Episode);
        let ids: Vec<_> = (rows.into_iter())
            .map(|row| {
                let id = primary_id(&ExternalIds {
                    tmdb: row.tmdb_id,
                    imdb: row.imdb_id,
                    tvdb: row.tvdb_id,
                });
                let show = primary_id(&ExternalIds {
                    tmdb: row.show_tmdb_id,
                    imdb: row.show_imdb_id,
                    tvdb: row.show_tvdb_id,
                });
                (id, show)
            })
            .collect();
        if holds_episodes {
            return Ok(MatchedTo::Episodes {
                show: ids.iter().find_map(|(_, show)| show.clone()),
                episodes: ids.into_iter().map(|(id, _)| id).collect(),
            });
        }
        Ok((ids.into_iter())
            .find_map(|(id, _)| id)
            .map_or(MatchedTo::Nothing, MatchedTo::Movie))
    }

    /// The path of the Version `version` and the file stored there, or `None` unless some Item of
    /// the file is Matched, so unmatched files are never served or probed.
    pub(crate) async fn version_file(
        &self,
        version: i64,
    ) -> Result<Option<(String, Stored)>, sqlx::Error> {
        let row = sqlx::query!(
            "SELECT path, size, modified_ns, parser_version FROM library_files
             WHERE id = ?
               AND EXISTS (SELECT 1 FROM library_items
                           JOIN item_matches ON item_matches.item_id = library_items.id
                           WHERE library_items.file_id = library_files.id
                             AND item_matches.state = 'matched')",
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

    pub(crate) async fn stores_image(
        &self,
        plugin: &str,
        image: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM item_matches
                              WHERE state = 'matched' AND plugin = ?
                                AND (poster = ?2 OR backdrop = ?2 OR still = ?2))
               AS "stored!: bool""#,
            plugin,
            image
        )
        .fetch_one(self.db.reader())
        .await
    }

    /// Each image a Match stores, as its Plugin and path, in the Library `library` or else in
    /// every Library.
    pub(crate) async fn images(
        &self,
        library: Option<i64>,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"WITH matched AS (
                   SELECT item_matches.plugin, item_matches.poster, item_matches.backdrop,
                          item_matches.still
                   FROM item_matches
                   JOIN library_items ON library_items.id = item_matches.item_id
                   JOIN library_files ON library_files.id = library_items.file_id
                   WHERE item_matches.state = 'matched'
                     AND (?1 IS NULL OR library_files.library_id = ?1))
               SELECT plugin AS "plugin!", poster AS "path!" FROM matched WHERE poster IS NOT NULL
               UNION
               SELECT plugin, backdrop FROM matched WHERE backdrop IS NOT NULL
               UNION
               SELECT plugin, still FROM matched WHERE still IS NOT NULL"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows.into_iter().map(|row| (row.plugin, row.path)).collect())
    }

    /// The downloaded Backdrop of the Matched title most recently added to each Library, of
    /// those with one, by Library id. A Library with no such title has none.
    pub(crate) async fn newest_backdrops(
        &self,
    ) -> Result<HashMap<i64, NewestBackdrop>, sqlx::Error> {
        // Item ids are AUTOINCREMENT, so the highest is the Item added last.
        let rows = sqlx::query!(
            r#"SELECT library_id AS "library!", plugin AS "plugin!", backdrop AS "backdrop!",
                      tmdb_id AS "tmdb_id?: i64", imdb_id AS "imdb_id?: String",
                      tvdb_id AS "tvdb_id?: i64"
               FROM (SELECT library_files.library_id, item_matches.plugin, item_matches.backdrop,
                            IIF(library_items.kind = 'movie', item_matches.tmdb_id,
                                item_matches.show_tmdb_id) AS tmdb_id,
                            IIF(library_items.kind = 'movie', item_matches.imdb_id,
                                item_matches.show_imdb_id) AS imdb_id,
                            IIF(library_items.kind = 'movie', item_matches.tvdb_id,
                                item_matches.show_tvdb_id) AS tvdb_id,
                            ROW_NUMBER() OVER (PARTITION BY library_files.library_id
                                               ORDER BY library_items.id DESC) AS newest
                     FROM item_matches
                     JOIN library_items ON library_items.id = item_matches.item_id
                     JOIN library_files ON library_files.id = library_items.file_id
                     WHERE item_matches.state = 'matched' AND item_matches.backdrop IS NOT NULL)
               WHERE newest = 1"#
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let ids = ExternalIds {
                    tmdb: row.tmdb_id,
                    imdb: row.imdb_id,
                    tvdb: row.tvdb_id,
                };
                let backdrop = NewestBackdrop {
                    title: primary_id(&ids)?,
                    plugin: row.plugin,
                    path: row.backdrop,
                };
                Some((row.library, backdrop))
            })
            .collect())
    }

    /// The oldest Library with an Item Matched to the Movie `id`, by any of its ids, if any.
    pub(crate) async fn library_with_movie(
        &self,
        id: &ExternalId,
    ) -> Result<Option<i64>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        library_with_movie(&mut conn, id).await
    }

    /// By title, grouped by [`primary_id`].
    pub(crate) async fn movies(&self, library: i64) -> Result<Vec<Movie>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_files.id AS "file!", library_files.path, library_files.size,
                      library_files.modified_ns, library_files.parser_version,
                      item_matches.plugin AS "plugin!", item_matches.tmdb_id,
                      item_matches.imdb_id, item_matches.tvdb_id, item_matches.title AS "title!",
                      item_matches.original_title, item_matches.overview,
                      item_matches.release_date, item_matches.runtime_minutes,
                      item_matches.poster, item_matches.backdrop
               FROM item_matches
               JOIN library_items ON library_items.id = item_matches.item_id
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?
                 AND library_items.kind = 'movie'
                 AND item_matches.state = 'matched'
               ORDER BY item_matches.tmdb_id,
                        IIF(item_matches.tmdb_id IS NULL, item_matches.imdb_id, NULL),
                        IIF(item_matches.tmdb_id IS NULL AND item_matches.imdb_id IS NULL,
                            item_matches.tvdb_id, NULL),
                        library_files.path"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        // Ordered by primary id, then path, so the rows of one Movie are adjacent, by path.
        let mut movies: Vec<Movie> = Vec::new();
        for row in rows {
            let ids = ExternalIds {
                tmdb: row.tmdb_id,
                imdb: row.imdb_id,
                tvdb: row.tvdb_id,
            };
            let Some(id) = primary_id(&ids) else {
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
                    plugin: row.plugin,
                    title: row.title,
                    original_title: row.original_title,
                    overview: row.overview,
                    release_date: row.release_date,
                    runtime_minutes: row.runtime_minutes,
                    poster: row.poster,
                    backdrop: row.backdrop,
                    versions: vec![version],
                }),
            }
        }
        movies.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(movies)
    }

    /// By title, grouped by the Episodes' Show ids' [`primary_id`], each Season's Episodes by
    /// their own.
    pub(crate) async fn shows(&self, library: i64) -> Result<Vec<Show>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_files.id AS "file!", library_files.path, library_files.size,
                      library_files.modified_ns, library_files.parser_version,
                      library_items.season AS "season!", library_items.episode AS "episode!",
                      item_matches.plugin AS "plugin!", item_matches.tmdb_id,
                      item_matches.imdb_id, item_matches.tvdb_id, item_matches.show_tmdb_id,
                      item_matches.show_imdb_id, item_matches.show_tvdb_id,
                      item_matches.title AS "title!", item_matches.show_title AS "show_title!",
                      item_matches.overview, item_matches.release_date,
                      item_matches.runtime_minutes, item_matches.poster, item_matches.backdrop,
                      item_matches.still
               FROM item_matches
               JOIN library_items ON library_items.id = item_matches.item_id
               JOIN library_files ON library_files.id = library_items.file_id
               WHERE library_files.library_id = ?
                 AND library_items.kind = 'episode'
                 AND item_matches.state = 'matched'
               ORDER BY library_items.season, library_items.episode, library_files.path"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        // Ordered by season, episode, then path, so within a Season the rows of one Episode are
        // adjacent, by path.
        let mut shows: BTreeMap<ExternalId, Show> = BTreeMap::new();
        for row in rows {
            let show_ids = ExternalIds {
                tmdb: row.show_tmdb_id,
                imdb: row.show_imdb_id,
                tvdb: row.show_tvdb_id,
            };
            let ids = ExternalIds {
                tmdb: row.tmdb_id,
                imdb: row.imdb_id,
                tvdb: row.tvdb_id,
            };
            let (Some(show_id), Some(id)) = (primary_id(&show_ids), primary_id(&ids)) else {
                continue;
            };
            let show = shows.entry(show_id.clone()).or_insert_with(|| Show {
                id: show_id,
                tvdb_id: row.show_tvdb_id,
                plugin: row.plugin.clone(),
                title: row.show_title,
                backdrop: row.backdrop,
                seasons: BTreeMap::new(),
            });
            let season = show.seasons.entry(row.season).or_insert_with(|| Season {
                poster: row.poster,
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
        let mut shows: Vec<Show> = shows.into_values().collect();
        shows.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(shows)
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
