//! What a Library holds: the video files a scan stored, the Items their names say they hold, what
//! Matching decided about each Item, and the Matches an admin pinned.
//!
//! [`Contents`] owns the tables (see the crate's `AGENTS.md`); each method is one read or one write.
//! A write that stores Matches also makes the Matched Movies' and Seasons' Requests Available, or
//! Partially Available (ADR 0012).

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use nama_plugin_api::{
    PluginId,
    metadata::{self, ExternalId, ExternalIds, TitleKind},
};
use nama_proto::nama::v1::{Event, LibraryItemAdded, LibraryItemRemoved};
use sqlx::SqliteConnection;

use crate::{
    db::{Database, now},
    events::event,
    requests,
    scanner::PARSER_VERSION,
    tracks::Track,
};

/// Reads and writes the contents of Libraries.
#[derive(Debug, Clone)]
pub(crate) struct Contents {
    db: Arc<Database>,
}

/// Something a file holds, as far as its path says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item {
    /// A Movie.
    Movie {
        /// The title, as the folder or file names it.
        title: String,
        /// The release year.
        year: Option<i64>,
        /// The Movie's id, from an ID Hint.
        hint: Option<ExternalId>,
    },
    /// One Episode of a Show.
    Episode {
        /// The Show's title, as its folder names it.
        show: String,
        /// The year the Show first aired.
        year: Option<i64>,
        /// The Show's id, from an ID Hint on its folder.
        hint: Option<ExternalId>,
        /// The Season number; 0 holds specials.
        season: i64,
        /// The Episode number within the Season.
        episode: i64,
    },
}

/// The name an Item goes by: a Movie's title, year and ID hint, or those of the Show an Episode
/// is in. Items of a Library named alike share a pin.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Name {
    kind: ItemKind,
    pub(crate) title: String,
    pub(crate) year: Option<i64>,
    pub(crate) hint: Option<ExternalId>,
}

/// A Match an admin set or cleared by hand, which Matching follows instead of searching.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pin {
    /// Match the Movie, or for Episodes the Show, with this id.
    Title(ExternalId),
    /// Leave the Items Unmatched.
    Unmatched,
}

/// The Items of a Library that share one Item's name.
#[derive(Debug)]
pub(crate) struct Named {
    pub(crate) library: i64,
    pub(crate) name: Name,
    pub(crate) items: Vec<(i64, Item)>,
}

#[derive(Debug)]
pub(crate) struct Unmatched {
    pub(crate) id: i64,
    pub(crate) path: String,
    pub(crate) item: Item,
    /// `None` while Matching hasn't decided, or for an Unmatched reason not stored.
    pub(crate) reason: Option<UnmatchedReason>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Stored {
    pub(crate) id: i64,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
    /// The version of the name parser that read it.
    pub(crate) parser_version: i64,
}

/// A change to a Library's stored files.
#[derive(Debug)]
pub(crate) enum Change {
    /// Store a file that's new or changed, with what it holds, replacing what was stored.
    Store {
        path: String,
        size: i64,
        modified_ns: i64,
        items: Vec<Item>,
    },
    /// Store what an unchanged file holds as the current name parser reads it. Its Items keep
    /// their ids and Matches, but not their automatic Unmatched decisions, so Matching tries them
    /// again (ADR 0023). A pin on an Item's old name also applies to its new one.
    Reparse {
        file: i64,
        path: String,
        items: Vec<Item>,
    },
    Remove(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decided {
    Matched(Box<Metadata>),
    Unmatched(Option<UnmatchedReason>),
}

/// Why automatic Matching left an Item Unmatched, which says how an admin fixes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnmatchedReason {
    /// More than one title fits about as well: these, best fit first.
    Ambiguous(Vec<TiedTitle>),
    /// No title the search found fits well enough.
    NoTitleFits,
    /// The Item's ID Hint names no title the metadata source has.
    UnknownIdHint,
}

/// A title that fit an Ambiguous Item about as well as the others.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TiedTitle {
    pub(crate) id: ExternalId,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) year: Option<i32>,
}

impl From<metadata::Candidate> for TiedTitle {
    fn from(candidate: metadata::Candidate) -> Self {
        Self {
            id: candidate.id,
            title: candidate.title,
            original_title: candidate.original_title,
            year: candidate.year,
        }
    }
}

/// What Matching decided about some Items, as one write stores it.
#[derive(Debug, Default)]
pub(crate) struct Decisions {
    /// Each Item, with what was decided about it.
    pub(crate) items: Vec<(i64, Decided)>,
    /// The Seasons fetched to decide about Episodes, as the metadata source gave them, with their
    /// Show's TMDB id. Their Requests follow how much of them a Library has.
    pub(crate) seasons: Vec<(i64, metadata::Season)>,
}

impl From<Vec<(i64, Decided)>> for Decisions {
    fn from(items: Vec<(i64, Decided)>) -> Self {
        Self {
            items,
            seasons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Metadata {
    /// The Plugin whose metadata Provider made the Match.
    pub(crate) plugin: PluginId,
    pub(crate) ids: ExternalIds,
    /// For an Episode, its Show's.
    pub(crate) show_ids: ExternalIds,
    pub(crate) title: String,
    pub(crate) original_title: Option<String>,
    pub(crate) show_title: Option<String>,
    pub(crate) overview: Option<String>,
    pub(crate) release_date: Option<String>,
    pub(crate) runtime_minutes: Option<u32>,
    pub(crate) poster: Option<String>,
    pub(crate) backdrop: Option<String>,
}

/// A Movie some Items of a Library are Matched to, with the metadata the Match of its first
/// Version stores.
#[derive(Debug)]
pub(crate) struct Movie {
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
    pub(crate) versions: Vec<Version>,
}

/// A stored file that a Movie or Episode is available in.
#[derive(Debug, Clone)]
pub(crate) struct Version {
    /// Its file as stored now: its Tracks count only if probing read it at this size and time.
    pub(crate) file: Stored,
    pub(crate) path: String,
    /// Empty as read here: Browse fills them in from [`crate::tracks::Tracks`], which owns them.
    pub(crate) tracks: Vec<Track>,
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
    /// Each Episode the file holds, by season and episode number: its id, as browsing identifies
    /// it, or `None` if its Item isn't Matched.
    Episodes(Vec<Option<ExternalId>>),
}

/// A Show some Items of a Library are Matched to Episodes of, with the metadata the Match of its
/// first Episode, by season, episode and path, stores.
#[derive(Debug, Clone)]
pub(crate) struct Show {
    /// Which of its Episodes' Show ids identifies it: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The Plugin whose metadata Provider made the Match, which the image paths are for.
    pub(crate) plugin: String,
    pub(crate) title: String,
    pub(crate) backdrop: Option<String>,
    /// The Seasons the Library has Episodes of, by number.
    pub(crate) seasons: BTreeMap<i64, Season>,
}

impl Show {
    /// The Show's poster: its first Season's, since a Match stores each Episode's Season poster
    /// and none of the Show's own.
    pub(crate) fn poster(&self) -> Option<&str> {
        let (_, first) = self.seasons.first_key_value()?;
        first.poster.as_deref()
    }

    /// Every Episode of every Season, by season and episode number.
    pub(crate) fn episodes(&self) -> impl Iterator<Item = &Episode> {
        self.seasons.values().flat_map(|season| &season.episodes)
    }
}

/// The Episodes of a Show's Season that some Items of a Library are Matched to.
#[derive(Debug, Clone)]
pub(crate) struct Season {
    /// The poster the Match of its first Episode stores.
    pub(crate) poster: Option<String>,
    /// By episode number.
    pub(crate) episodes: Vec<Episode>,
}

/// An Episode some Items of a Library are Matched to, with the metadata the Match of its first
/// Version stores.
#[derive(Debug, Clone)]
pub(crate) struct Episode {
    /// Which of the Match's ids identifies the Episode: see [`primary_id`].
    pub(crate) id: ExternalId,
    /// The episode number within its Season.
    pub(crate) number: i64,
    pub(crate) title: String,
    pub(crate) overview: Option<String>,
    pub(crate) air_date: Option<String>,
    pub(crate) runtime_minutes: Option<i64>,
    /// The files Matched to it, by path. A file holding two Episodes is a Version of each.
    pub(crate) versions: Vec<Version>,
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
    /// The best-fitting Episode's Season's.
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

    pub(crate) async fn stored(
        &self,
        library: i64,
    ) -> Result<HashMap<String, Stored>, sqlx::Error> {
        Ok(sqlx::query!(
            r#"SELECT id AS "id!", path, size, modified_ns, parser_version FROM library_files
               WHERE library_id = ?"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?
        .into_iter()
        .map(|row| {
            let stored = Stored {
                id: row.id,
                size: row.size,
                modified_ns: row.modified_ns,
                parser_version: row.parser_version,
            };
            (row.path, stored)
        })
        .collect())
    }

    /// Every file stored, in every Library.
    pub(crate) async fn every_stored(&self) -> Result<Vec<Stored>, sqlx::Error> {
        sqlx::query_as!(
            Stored,
            r#"SELECT id AS "id!", size, modified_ns, parser_version FROM library_files"#
        )
        .fetch_all(self.db.reader())
        .await
    }

    /// Returns the events for the Items `changes` removes and adds. Replacing an Item removes what
    /// Matching decided about it.
    pub(crate) async fn apply(
        &self,
        library: i64,
        changes: Vec<Change>,
    ) -> Result<Vec<Event>, sqlx::Error> {
        self.db
            .write(move |tx| Box::pin(apply(tx, library, changes)))
            .await
    }

    /// The Items of the Library `library` that Matching hasn't decided about, with what an admin
    /// pinned for their names, in the order they were stored.
    pub(crate) async fn undecided(
        &self,
        library: i64,
    ) -> Result<Vec<(i64, Item, Option<Pin>)>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_items.kind AS "kind: ItemKind",
                      library_items.title, library_items.year, library_items.hint,
                      library_items.season, library_items.episode,
                      match_pins.pinned_at IS NOT NULL AS "pinned!: bool",
                      match_pins.pinned_tmdb_id, match_pins.pinned_imdb_id,
                      match_pins.pinned_tvdb_id
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN match_pins ON match_pins.library_id = library_files.library_id
                                   AND match_pins.kind = library_items.kind
                                   AND match_pins.title = library_items.title
                                   AND match_pins.year IS library_items.year
                                   AND match_pins.hint IS library_items.hint
               WHERE library_files.library_id = ?
                 AND NOT EXISTS (SELECT 1 FROM item_matches
                                 WHERE item_matches.item_id = library_items.id)
               ORDER BY library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let pinned = ExternalIds {
                    tmdb: row.pinned_tmdb_id,
                    imdb: row.pinned_imdb_id,
                    tvdb: row.pinned_tvdb_id,
                };
                let item = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((row.id, item, Pin::from_row(row.pinned, pinned)))
            })
            .collect())
    }

    /// Stores what was `decided` about each Item, as of now, skipping Items that no longer exist.
    /// Returns the events for the Requests the Matches make Available or Partially Available.
    pub(crate) async fn record(
        &self,
        decided: impl Into<Decisions>,
    ) -> Result<Vec<Event>, sqlx::Error> {
        let decided = decided.into();
        let now = now();
        self.db
            .write(move |tx| Box::pin(insert_decisions(tx, decided, now)))
            .await
    }

    pub(crate) async fn library_of(&self, item: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT library_files.library_id
             FROM library_items
             JOIN library_files ON library_files.id = library_items.file_id
             WHERE library_items.id = ?",
            item
        )
        .fetch_optional(self.db.reader())
        .await
    }

    pub(crate) async fn matched_to(&self, version: i64) -> Result<MatchedTo, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.kind AS "kind?: ItemKind", item_matches.tmdb_id, item_matches.imdb_id,
                      item_matches.tvdb_id
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
        let mut ids = rows.into_iter().map(|row| {
            primary_id(ExternalIds {
                tmdb: row.tmdb_id,
                imdb: row.imdb_id,
                tvdb: row.tvdb_id,
            })
        });
        if holds_episodes {
            return Ok(MatchedTo::Episodes(ids.collect()));
        }
        Ok(ids
            .find_map(|id| id)
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
                                AND (poster = ? OR backdrop = ?)) AS "stored!: bool""#,
            plugin,
            image,
            image
        )
        .fetch_one(self.db.reader())
        .await
    }

    /// The oldest Library with an Item Matched to the Movie with the TMDB id `tmdb_id`, if any.
    pub(crate) async fn library_with_movie(
        &self,
        tmdb_id: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        let mut conn = self.db.reader().acquire().await?;
        library_with_movie(&mut conn, tmdb_id).await
    }

    /// The Items of the Item `item`'s Library named like it, itself among them, or `None` if
    /// there is no such Item.
    pub(crate) async fn named_like(&self, item: i64) -> Result<Option<Named>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT files.library_id, named.id AS "id!", named.kind AS "kind: ItemKind",
                      named.title, named.year, named.hint, named.season, named.episode
               FROM library_items AS this
               JOIN library_files AS this_file ON this_file.id = this.file_id
               JOIN library_files AS files ON files.library_id = this_file.library_id
               JOIN library_items AS named ON named.file_id = files.id
               WHERE this.id = ?
                 AND named.kind = this.kind
                 AND named.title = this.title
                 AND named.year IS this.year
                 AND named.hint IS this.hint
               ORDER BY named.id"#,
            item
        )
        .fetch_all(self.db.reader())
        .await?;
        let Some(library) = rows.first().map(|row| row.library_id) else {
            return Ok(None);
        };
        let items: Vec<(i64, Item)> = rows
            .into_iter()
            .filter_map(|row| {
                let alike = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((row.id, alike))
            })
            .collect();
        let Some(name) = items.first().map(|(_, first)| first.name()) else {
            return Ok(None);
        };
        Ok(Some(Named {
            library,
            name,
            items,
        }))
    }

    /// The Items of the Library `library` that need a Match, by path: those Matching left
    /// Unmatched or hasn't decided about, except those pinned Unmatched.
    pub(crate) async fn unmatched(&self, library: i64) -> Result<Vec<Unmatched>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_items.id AS "id!", library_files.path,
                      library_items.kind AS "kind: ItemKind", library_items.title,
                      library_items.year, library_items.hint, library_items.season,
                      library_items.episode,
                      item_matches.unmatched_reason AS "unmatched_reason?: ReasonColumn",
                      item_matches.tied_titles AS "tied_titles?"
               FROM library_items
               JOIN library_files ON library_files.id = library_items.file_id
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
               WHERE library_files.library_id = ?
                 AND item_matches.state IS NOT 'matched'
                 AND NOT EXISTS (SELECT 1 FROM match_pins
                                 WHERE match_pins.library_id = library_files.library_id
                                   AND match_pins.kind = library_items.kind
                                   AND match_pins.title = library_items.title
                                   AND match_pins.year IS library_items.year
                                   AND match_pins.hint IS library_items.hint
                                   AND match_pins.pinned_tmdb_id IS NULL
                                   AND match_pins.pinned_imdb_id IS NULL
                                   AND match_pins.pinned_tvdb_id IS NULL)
               ORDER BY library_files.path, library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        rows.into_iter()
            .filter_map(|row| {
                let reason = row
                    .unmatched_reason
                    .map(|column| column.reason(row.tied_titles.as_deref()))
                    .transpose();
                let item = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    hint: row.hint,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some(reason.map(|reason| Unmatched {
                    id: row.id,
                    path: row.path,
                    item,
                    reason,
                }))
            })
            .collect()
    }

    /// Files whose names don't follow the naming conventions, so they hold no Items, by path.
    pub(crate) async fn unparsed(&self, library: i64) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar!(
            "SELECT path FROM library_files
             WHERE library_id = ?
               AND NOT EXISTS (SELECT 1 FROM library_items
                               WHERE library_items.file_id = library_files.id)
             ORDER BY path",
            library
        )
        .fetch_all(self.db.reader())
        .await
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
            let Some(id) = primary_id(ids) else {
                continue;
            };
            let version = Version {
                file: Stored {
                    id: row.file,
                    size: row.size,
                    modified_ns: row.modified_ns,
                    parser_version: row.parser_version,
                },
                path: row.path,
                tracks: Vec::new(),
            };
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
                      item_matches.runtime_minutes, item_matches.poster, item_matches.backdrop
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
            let (Some(show_id), Some(id)) = (primary_id(show_ids), primary_id(ids)) else {
                continue;
            };
            let show = shows.entry(show_id.clone()).or_insert_with(|| Show {
                id: show_id,
                plugin: row.plugin,
                title: row.show_title,
                backdrop: row.backdrop,
                seasons: BTreeMap::new(),
            });
            let season = show.seasons.entry(row.season).or_insert_with(|| Season {
                poster: row.poster,
                episodes: Vec::new(),
            });
            let version = Version {
                file: Stored {
                    id: row.file,
                    size: row.size,
                    modified_ns: row.modified_ns,
                    parser_version: row.parser_version,
                },
                path: row.path,
                tracks: Vec::new(),
            };
            match season.episodes.last_mut() {
                Some(episode) if episode.id == id => episode.versions.push(version),
                _ => season.episodes.push(Episode {
                    id,
                    number: row.episode,
                    title: row.title,
                    overview: row.overview,
                    air_date: row.release_date,
                    runtime_minutes: row.runtime_minutes,
                    versions: vec![version],
                }),
            }
        }
        let mut shows: Vec<Show> = shows.into_values().collect();
        shows.sort_by(|a, b| a.title.cmp(&b.title));
        Ok(shows)
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
        for row in self.search_rows(&words.in_column("show_title")).await? {
            let (Some(id), Some(title)) = (primary_id(row.show_ids()), row.show_title) else {
                continue;
            };
            found.shows.push(FoundShow {
                library: row.library,
                id,
                plugin: row.plugin,
                title,
                poster: row.poster,
            });
        }
        // A Movie, Show or Episode has a row per Item; the first fits best.
        let mut seen = HashSet::new();
        found
            .movies
            .retain(|movie| seen.insert((movie.library, movie.id.clone())));
        seen.clear();
        found
            .shows
            .retain(|show| seen.insert((show.library, show.id.clone())));
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

    /// Replaces any pin the Items called `name` had, in one write with `decided`. Returns the
    /// events for the Requests the Matches make Available or Partially Available, or `None`,
    /// having stored nothing, if the Library is gone.
    pub(crate) async fn pin(
        &self,
        library: i64,
        name: Name,
        pin: Pin,
        decided: impl Into<Decisions>,
    ) -> Result<Option<Vec<Event>>, sqlx::Error> {
        let decided = decided.into();
        self.db
            .write(move |tx| Box::pin(store_pin(tx, library, name, pin, decided)))
            .await
    }
}

impl Item {
    pub(crate) fn name(&self) -> Name {
        let (Self::Movie { hint, .. } | Self::Episode { hint, .. }) = self;
        let row = ItemRow::from(self);
        Name {
            kind: row.kind,
            title: row.title.into_owned(),
            year: row.year,
            hint: hint.clone(),
        }
    }
}

impl Name {
    pub(crate) const fn kind(&self) -> TitleKind {
        match self.kind {
            ItemKind::Movie => TitleKind::Movie,
            ItemKind::Episode => TitleKind::Show,
        }
    }
}

impl Pin {
    /// The pin stored in a `match_pins` row, if there is one, holding the id in `ids`.
    fn from_row(pinned: bool, ids: ExternalIds) -> Option<Self> {
        if !pinned {
            return None;
        }
        Some(primary_id(ids).map_or(Self::Unmatched, Self::Title))
    }

    fn columns(&self) -> ExternalIds {
        let mut ids = ExternalIds::default();
        match self {
            Self::Title(ExternalId::Tmdb(id)) => ids.tmdb = Some(*id),
            Self::Title(ExternalId::Imdb(id)) => ids.imdb = Some(id.clone()),
            Self::Title(ExternalId::Tvdb(id)) => ids.tvdb = Some(*id),
            Self::Unmatched => {}
        }
        ids
    }
}

/// The oldest Library with an Item Matched to the Movie with the TMDB id `tmdb_id`, if any, read
/// on `conn` so a write can act on the answer before a scan changes it.
pub(crate) async fn library_with_movie(
    conn: &mut SqliteConnection,
    tmdb_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT MIN(library_files.library_id) AS "library: i64"
           FROM item_matches
           JOIN library_items ON library_items.id = item_matches.item_id
           JOIN library_files ON library_files.id = library_items.file_id
           WHERE item_matches.state = 'matched'
             AND library_items.kind = 'movie'
             AND item_matches.tmdb_id = ?"#,
        tmdb_id
    )
    .fetch_one(&mut *conn)
    .await
}

/// The numbers of the Episodes of Season `season` of the Show with the TMDB id `show_tmdb_id`
/// Matched in any Library.
pub(crate) async fn matched_episodes(
    conn: &mut SqliteConnection,
    show_tmdb_id: i64,
    season: i64,
) -> Result<HashSet<i64>, sqlx::Error> {
    // Matching finds an Episode by the numbers its file's name gives, so they are the Episode's.
    let numbers = sqlx::query_scalar!(
        r#"SELECT library_items.episode AS "episode!"
           FROM item_matches
           JOIN library_items ON library_items.id = item_matches.item_id
           WHERE item_matches.state = 'matched'
             AND library_items.kind = 'episode'
             AND item_matches.show_tmdb_id = ?
             AND library_items.season = ?"#,
        show_tmdb_id,
        season
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(numbers.into_iter().collect())
}

/// The one of `ids` that identifies a title: its TMDB id if it has one, or else its IMDb or TVDB
/// id. `None` if it has none.
fn primary_id(ids: ExternalIds) -> Option<ExternalId> {
    ids.tmdb
        .map(ExternalId::Tmdb)
        .or_else(|| ids.imdb.map(ExternalId::Imdb))
        .or_else(|| ids.tvdb.map(ExternalId::Tvdb))
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

/// The ID Hint stored as `text`, which is how [`ExternalId`] displays.
fn parse_hint(text: &str) -> Option<ExternalId> {
    match text.split_once('-')? {
        ("tmdb", id) => id.parse().ok().map(ExternalId::Tmdb),
        ("tvdb", id) => id.parse().ok().map(ExternalId::Tvdb),
        ("imdb", id) => Some(ExternalId::Imdb(id.to_owned())),
        _ => None,
    }
}

async fn apply(
    conn: &mut SqliteConnection,
    library: i64,
    changes: Vec<Change>,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut events = Vec::new();
    for change in changes {
        match change {
            Change::Store {
                path,
                size,
                modified_ns,
                items,
            } => {
                let file = sqlx::query_scalar!(
                    r#"INSERT INTO library_files (library_id, path, size, modified_ns, parser_version)
                       VALUES (?, ?, ?, ?, ?)
                       ON CONFLICT (library_id, path)
                       DO UPDATE SET size = excluded.size, modified_ns = excluded.modified_ns,
                                     parser_version = excluded.parser_version
                       RETURNING id AS "id!""#,
                    library,
                    path,
                    size,
                    modified_ns,
                    PARSER_VERSION,
                )
                .fetch_one(&mut *conn)
                .await?;
                let removed = remove_items(conn, file).await?;
                let added = insert_items(conn, file, &items).await?;
                events.extend(removed.into_iter().map(|item| item_removed(library, item)));
                events.extend(
                    added
                        .into_iter()
                        .map(|item| item_added(library, item, path.clone())),
                );
            }
            Change::Reparse { file, path, items } => {
                events.extend(reparse(conn, library, file, &path, &items).await?);
            }
            Change::Remove(file) => {
                let removed = remove_items(conn, file).await?;
                sqlx::query!("DELETE FROM library_files WHERE id = ?", file)
                    .execute(&mut *conn)
                    .await?;
                events.extend(removed.into_iter().map(|item| item_removed(library, item)));
            }
        }
    }
    Ok(events)
}

/// Pairs each of `items` with the stored Item of the file it was parsed as before, by Season and
/// Episode number, so it keeps its id and Match; the unpaired are removed or added.
async fn reparse(
    conn: &mut SqliteConnection,
    library: i64,
    file: i64,
    path: &str,
    items: &[Item],
) -> Result<Vec<Event>, sqlx::Error> {
    sqlx::query!(
        "UPDATE library_files SET parser_version = ? WHERE id = ?",
        PARSER_VERSION,
        file
    )
    .execute(&mut *conn)
    .await?;
    let mut stored = sqlx::query!(
        r#"SELECT id AS "id!", kind AS "kind: ItemKind", title, year, hint, season, episode
           FROM library_items WHERE file_id = ? ORDER BY id"#,
        file
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut unpaired = Vec::new();
    for item in items {
        let new = ItemRow::from(item);
        let paired = stored
            .iter()
            .position(|old| (old.season, old.episode) == (new.season, new.episode));
        let Some(old) = paired.map(|at| stored.swap_remove(at)) else {
            unpaired.push(item.clone());
            continue;
        };
        let old_row = ItemRow {
            kind: old.kind,
            title: Cow::Owned(old.title),
            year: old.year,
            hint: old.hint,
            season: old.season,
            episode: old.episode,
        };
        rename_item(conn, library, old.id, &old_row, &new).await?;
    }
    let mut events = Vec::new();
    for gone in stored {
        sqlx::query!("DELETE FROM library_items WHERE id = ?", gone.id)
            .execute(&mut *conn)
            .await?;
        events.push(item_removed(library, gone.id));
    }
    let added = insert_items(conn, file, &unpaired).await?;
    events.extend(
        added
            .into_iter()
            .map(|item| item_added(library, item, path.to_owned())),
    );
    sqlx::query!(
        "DELETE FROM item_matches
         WHERE state = 'unmatched'
           AND item_id IN (SELECT id FROM library_items WHERE file_id = ?)",
        file
    )
    .execute(&mut *conn)
    .await?;
    Ok(events)
}

/// Stores `new` as what the Item `id` holds, and gives its name any pin on `old`.
async fn rename_item(
    conn: &mut SqliteConnection,
    library: i64,
    id: i64,
    old: &ItemRow<'_>,
    new: &ItemRow<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE library_items SET kind = ?, title = ?, year = ?, hint = ?, season = ?,
                                  episode = ?
         WHERE id = ?",
        new.kind,
        new.title,
        new.year,
        new.hint,
        new.season,
        new.episode,
        id,
    )
    .execute(&mut *conn)
    .await?;
    // A pin is keyed by name, so a new name would lose it. The old pin stays for any Item
    // still named the old way; a name that is already pinned keeps its own pin.
    sqlx::query!(
        "INSERT OR IGNORE INTO match_pins (
             library_id, kind, title, year, hint,
             pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at)
         SELECT library_id, kind, ?, ?, ?,
                pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at
         FROM match_pins
         WHERE library_id = ? AND kind = ? AND title = ? AND year IS ? AND hint IS ?",
        new.title,
        new.year,
        new.hint,
        library,
        old.kind,
        old.title,
        old.year,
        old.hint,
    )
    .execute(conn)
    .await?;
    Ok(())
}

fn item_added(library: i64, item: i64, path: String) -> Event {
    event(LibraryItemAdded {
        library_id: library,
        item_id: item,
        path,
        ..Default::default()
    })
}

fn item_removed(library: i64, item: i64) -> Event {
    event(LibraryItemRemoved {
        library_id: library,
        item_id: item,
        ..Default::default()
    })
}

async fn remove_items(conn: &mut SqliteConnection, file: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"DELETE FROM library_items WHERE file_id = ? RETURNING id AS "id!""#,
        file
    )
    .fetch_all(conn)
    .await
}

async fn insert_items(
    conn: &mut SqliteConnection,
    file: i64,
    items: &[Item],
) -> Result<Vec<i64>, sqlx::Error> {
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        let row = ItemRow::from(item);
        let id = sqlx::query_scalar!(
            r#"INSERT INTO library_items (file_id, kind, title, year, hint, season, episode)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               RETURNING id AS "id!""#,
            file,
            row.kind,
            row.title,
            row.year,
            row.hint,
            row.season,
            row.episode,
        )
        .fetch_one(&mut *conn)
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

async fn store_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: Name,
    pin: Pin,
    decided: Decisions,
) -> Result<Option<Vec<Event>>, sqlx::Error> {
    let now = now();
    if !insert_pin(conn, library, &name, &pin, now).await? {
        return Ok(None);
    }
    insert_decisions(conn, decided, now).await.map(Some)
}

/// Returns whether it was stored: it isn't if the Library is gone.
async fn insert_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: &Name,
    pin: &Pin,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let pinned = pin.columns();
    let hint = name.hint.as_ref().map(ExternalId::to_string);
    let inserted = sqlx::query!(
        "INSERT OR REPLACE INTO match_pins (
             library_id, kind, title, year, hint,
             pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at)
         SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?
         WHERE EXISTS (SELECT 1 FROM libraries WHERE id = ?)",
        library,
        name.kind,
        name.title,
        name.year,
        hint,
        pinned.tmdb,
        pinned.imdb,
        pinned.tvdb,
        now,
        library,
    )
    .execute(conn)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

/// Returns the events for the Requests the Matches make Available or Partially Available, made in
/// the same write so no Request stays open for a Movie or Season a Library has.
async fn insert_decisions(
    conn: &mut SqliteConnection,
    decided: Decisions,
    now: i64,
) -> Result<Vec<Event>, sqlx::Error> {
    let mut tmdb_ids = Vec::new();
    for (item, decision) in decided.items {
        if let Decided::Matched(metadata) = &decision {
            tmdb_ids.extend(metadata.ids.tmdb);
        }
        let row = MatchRow::from(&decision);
        sqlx::query!(
            "INSERT OR REPLACE INTO item_matches (
                 item_id, state, plugin, tmdb_id, imdb_id, tvdb_id,
                 show_tmdb_id, show_imdb_id, show_tvdb_id, title, original_title, show_title,
                 overview, release_date, runtime_minutes, poster, backdrop, unmatched_reason,
                 tied_titles, decided_at)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
             WHERE EXISTS (SELECT 1 FROM library_items WHERE id = ?)",
            item,
            row.state,
            row.plugin,
            row.ids.tmdb,
            row.ids.imdb,
            row.ids.tvdb,
            row.show_ids.tmdb,
            row.show_ids.imdb,
            row.show_ids.tvdb,
            row.metadata.title,
            row.metadata.original_title,
            row.metadata.show_title,
            row.metadata.overview,
            row.metadata.release_date,
            row.metadata.runtime_minutes,
            row.metadata.poster,
            row.metadata.backdrop,
            row.unmatched_reason,
            row.tied_titles,
            now,
            item,
        )
        .execute(&mut *conn)
        .await?;
    }
    let mut happened = requests::make_available(conn, tmdb_ids).await?;
    happened.extend(requests::make_seasons_available(conn, decided.seasons).await?);
    Ok(happened)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
enum ItemKind {
    Movie,
    Episode,
}

#[derive(Debug)]
struct ItemRow<'a> {
    kind: ItemKind,
    title: Cow<'a, str>,
    year: Option<i64>,
    /// An [`ExternalId`] as it displays, such as `tvdb-433631`.
    hint: Option<String>,
    season: Option<i64>,
    episode: Option<i64>,
}

impl ItemRow<'_> {
    /// The Item the row holds. `None` for an Episode without its numbers, which the schema
    /// forbids.
    fn into_item(self) -> Option<Item> {
        let Self {
            kind,
            title,
            year,
            hint,
            season,
            episode,
        } = self;
        let hint = hint.as_deref().and_then(parse_hint);
        Some(match kind {
            ItemKind::Movie => Item::Movie {
                title: title.into_owned(),
                year,
                hint,
            },
            ItemKind::Episode => Item::Episode {
                show: title.into_owned(),
                year,
                hint,
                season: season?,
                episode: episode?,
            },
        })
    }
}

impl<'a> From<&'a Item> for ItemRow<'a> {
    fn from(item: &'a Item) -> Self {
        match *item {
            Item::Movie {
                ref title,
                year,
                ref hint,
            } => Self {
                kind: ItemKind::Movie,
                title: Cow::Borrowed(title),
                year,
                hint: hint.as_ref().map(ExternalId::to_string),
                season: None,
                episode: None,
            },
            Item::Episode {
                ref show,
                year,
                ref hint,
                season,
                episode,
            } => Self {
                kind: ItemKind::Episode,
                title: Cow::Borrowed(show),
                year,
                hint: hint.as_ref().map(ExternalId::to_string),
                season: Some(season),
                episode: Some(episode),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
enum MatchState {
    Matched,
    Unmatched,
}

/// An [`UnmatchedReason`] as `item_matches.unmatched_reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
enum ReasonColumn {
    Ambiguous,
    NoTitleFits,
    UnknownIdHint,
}

impl ReasonColumn {
    const fn of(reason: &UnmatchedReason) -> Self {
        match reason {
            UnmatchedReason::Ambiguous(_) => Self::Ambiguous,
            UnmatchedReason::NoTitleFits => Self::NoTitleFits,
            UnmatchedReason::UnknownIdHint => Self::UnknownIdHint,
        }
    }

    /// The reason this column and `tied_titles`, the JSON the migration describes, store.
    fn reason(self, tied_titles: Option<&str>) -> Result<UnmatchedReason, sqlx::Error> {
        Ok(match self {
            Self::Ambiguous => UnmatchedReason::Ambiguous(parse_tied(tied_titles.unwrap_or("[]"))?),
            Self::NoTitleFits => UnmatchedReason::NoTitleFits,
            Self::UnknownIdHint => UnmatchedReason::UnknownIdHint,
        })
    }
}

fn parse_tied(json: &str) -> Result<Vec<TiedTitle>, sqlx::Error> {
    let decode = |err: Box<dyn std::error::Error + Send + Sync>| sqlx::Error::Decode(err);
    let rows: Vec<(String, String, Option<String>, Option<i32>)> =
        serde_json::from_str(json).map_err(|err| decode(err.into()))?;
    rows.into_iter()
        .map(|(id, title, original_title, year)| {
            let id =
                parse_hint(&id).ok_or_else(|| decode(format!("bad tied title id {id}").into()))?;
            Ok(TiedTitle {
                id,
                title,
                original_title,
                year,
            })
        })
        .collect()
}

fn tied_json(tied: &[TiedTitle]) -> String {
    let rows: Vec<serde_json::Value> = tied
        .iter()
        .map(|t| serde_json::json!([t.id.to_string(), t.title, t.original_title, t.year]))
        .collect();
    serde_json::Value::from(rows).to_string()
}

#[derive(Debug)]
struct MatchRow<'a> {
    state: MatchState,
    plugin: Option<&'a str>,
    ids: ExternalIds,
    show_ids: ExternalIds,
    metadata: MetadataColumns<'a>,
    unmatched_reason: Option<ReasonColumn>,
    tied_titles: Option<String>,
}

/// A Match's metadata as columns of `item_matches`; all empty for an Unmatched Item.
#[derive(Debug, Default)]
struct MetadataColumns<'a> {
    title: Option<&'a str>,
    original_title: Option<&'a str>,
    show_title: Option<&'a str>,
    overview: Option<&'a str>,
    release_date: Option<&'a str>,
    runtime_minutes: Option<u32>,
    poster: Option<&'a str>,
    backdrop: Option<&'a str>,
}

impl<'a> From<&'a Decided> for MatchRow<'a> {
    fn from(decided: &'a Decided) -> Self {
        match decided {
            Decided::Unmatched(reason) => Self {
                state: MatchState::Unmatched,
                plugin: None,
                ids: ExternalIds::default(),
                show_ids: ExternalIds::default(),
                metadata: MetadataColumns::default(),
                unmatched_reason: reason.as_ref().map(ReasonColumn::of),
                tied_titles: match reason {
                    Some(UnmatchedReason::Ambiguous(tied)) => Some(tied_json(tied)),
                    Some(UnmatchedReason::NoTitleFits | UnmatchedReason::UnknownIdHint) | None => {
                        None
                    }
                },
            },
            Decided::Matched(metadata) => Self {
                state: MatchState::Matched,
                plugin: Some(metadata.plugin.as_str()),
                ids: metadata.ids.clone(),
                show_ids: metadata.show_ids.clone(),
                metadata: MetadataColumns {
                    title: Some(&metadata.title),
                    original_title: metadata.original_title.as_deref(),
                    show_title: metadata.show_title.as_deref(),
                    overview: metadata.overview.as_deref(),
                    release_date: metadata.release_date.as_deref(),
                    runtime_minutes: metadata.runtime_minutes,
                    poster: metadata.poster.as_deref(),
                    backdrop: metadata.backdrop.as_deref(),
                },
                unmatched_reason: None,
                tied_titles: None,
            },
        }
    }
}

/// A stored file, with its Items and what Matching stored for each, as tests read them back.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct File {
    /// The absolute path.
    pub(crate) path: String,
    /// The size, in bytes.
    pub(crate) size: i64,
    /// What the file holds, in the order stored.
    pub(crate) items: Vec<FileItem>,
}

/// An Item of a [`File`], as tests read it back.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileItem {
    /// The Item's id.
    pub(crate) id: i64,
    /// What it is.
    pub(crate) item: Item,
    /// What Matching stored for it, or `None` while it's undecided.
    pub(crate) recorded: Option<Recorded>,
}

/// What Matching stored for an Item, as tests read it back.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recorded {
    /// Whether the Item is Matched, rather than Unmatched.
    pub(crate) matched: bool,
    /// The id of the Plugin whose metadata Provider made the Match.
    pub(crate) plugin: Option<String>,
    /// The Movie's or Episode's external ids.
    pub(crate) ids: ExternalIds,
    /// For an Episode, its Show's external ids.
    pub(crate) show_ids: ExternalIds,
    /// The Movie's or Episode's title.
    pub(crate) title: Option<String>,
    /// For an Episode, its Show's title.
    pub(crate) show_title: Option<String>,
    /// The poster's image path.
    pub(crate) poster: Option<String>,
}

#[cfg(test)]
impl Contents {
    /// The files stored for the Library `library`, by path, with their Items and what Matching
    /// stored for each.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub(crate) async fn files(&self, library: i64) -> Result<Vec<File>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_files.path, library_files.size,
                      library_items.id AS "item?", library_items.kind AS "kind?: ItemKind",
                      library_items.title AS "item_title?", library_items.year,
                      library_items.hint, library_items.season,
                      library_items.episode,
                      item_matches.state AS "state?: MatchState", item_matches.plugin,
                      item_matches.tmdb_id, item_matches.imdb_id, item_matches.tvdb_id,
                      item_matches.show_tmdb_id, item_matches.show_imdb_id,
                      item_matches.show_tvdb_id, item_matches.title, item_matches.show_title,
                      item_matches.poster
               FROM library_files
               LEFT JOIN library_items ON library_items.file_id = library_files.id
               LEFT JOIN item_matches ON item_matches.item_id = library_items.id
               WHERE library_files.library_id = ?
               ORDER BY library_files.path, library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        let mut files: Vec<File> = Vec::new();
        for row in rows {
            if files.last().is_none_or(|file| file.path != row.path) {
                files.push(File {
                    path: row.path.clone(),
                    size: row.size,
                    items: Vec::new(),
                });
            }
            let (Some(id), Some(kind), Some(title)) = (row.item, row.kind, row.item_title) else {
                continue;
            };
            let item = ItemRow {
                kind,
                title: Cow::Owned(title),
                year: row.year,
                hint: row.hint,
                season: row.season,
                episode: row.episode,
            }
            .into_item();
            let recorded = row.state.map(|state| Recorded {
                matched: state == MatchState::Matched,
                plugin: row.plugin,
                ids: ExternalIds {
                    tmdb: row.tmdb_id,
                    imdb: row.imdb_id,
                    tvdb: row.tvdb_id,
                },
                show_ids: ExternalIds {
                    tmdb: row.show_tmdb_id,
                    imdb: row.show_imdb_id,
                    tvdb: row.show_tvdb_id,
                },
                title: row.title,
                show_title: row.show_title,
                poster: row.poster,
            });
            if let (Some(file), Some(item)) = (files.last_mut(), item) {
                file.items.push(FileItem { id, item, recorded });
            }
        }
        Ok(files)
    }
}

#[cfg(test)]
impl Contents {
    /// Marks the files of the Library `library` as read by the name parser before the current
    /// one, as after a version bump.
    pub(crate) async fn read_by_older_parser(&self, library: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE library_files SET parser_version = ? - 1 WHERE library_id = ?",
                        PARSER_VERSION,
                        library
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use nama_plugin_api::{
        PluginId,
        metadata::{ExternalId, ExternalIds},
    };
    use tempfile::TempDir;

    use super::{Change, Contents, Decided, FileItem, Item, Metadata, Pin};
    use crate::{
        db::Database,
        libraries::{Kind, Libraries},
    };

    const ALIEN: &str = "/media/Alien (1979)/Alien (1979).mkv";

    /// A Movies Library with nothing stored yet.
    struct Fixture {
        contents: Contents,
        libraries: Libraries,
        library: i64,
        _folder: TempDir,
        _data: TempDir,
    }

    impl Fixture {
        async fn new() -> Self {
            let data = tempfile::tempdir().unwrap();
            let folder = tempfile::tempdir().unwrap();
            let db = Arc::new(Database::open(data.path()).await.unwrap());
            let libraries = Libraries::new(Arc::clone(&db));
            let root = folder.path().canonicalize().unwrap();
            let library = libraries
                .create(
                    "Movies",
                    Kind::Movies,
                    vec![root.to_str().unwrap().to_owned()],
                )
                .await
                .unwrap()
                .id;
            Self {
                contents: Contents::new(db),
                libraries,
                library,
                _folder: folder,
                _data: data,
            }
        }

        async fn store(&self, path: &str, size: i64, item: Item) {
            let change = Change::Store {
                path: path.to_owned(),
                size,
                modified_ns: 0,
                items: vec![item],
            };
            self.contents
                .apply(self.library, vec![change])
                .await
                .unwrap();
        }

        async fn items(&self, path: &str) -> Vec<FileItem> {
            self.contents
                .files(self.library)
                .await
                .unwrap()
                .into_iter()
                .find(|file| file.path == path)
                .unwrap()
                .items
        }

        async fn item(&self, path: &str) -> i64 {
            self.items(path).await[0].id
        }

        async fn undecided(&self) -> Vec<i64> {
            self.contents
                .undecided(self.library)
                .await
                .unwrap()
                .into_iter()
                .map(|(id, _, _)| id)
                .collect()
        }
    }

    fn movie(title: &str, year: i64) -> Item {
        Item::Movie {
            title: title.to_owned(),
            year: Some(year),
            hint: None,
        }
    }

    #[tokio::test]
    async fn storing_a_changed_file_replaces_its_items_and_their_matches_go_with_them() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let old = fx.item(ALIEN).await;
        fx.contents
            .record(vec![(old, Decided::Unmatched(None))])
            .await
            .unwrap();

        fx.store(ALIEN, 20, movie("Alien", 1979)).await;

        let items = fx.items(ALIEN).await;
        assert_eq!(
            items
                .iter()
                .map(|stored| (stored.id == old, stored.recorded.clone()))
                .collect::<Vec<_>>(),
            [(false, None)]
        );
    }

    #[tokio::test]
    async fn removing_a_file_deletes_it_with_its_items_and_their_matches() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let item = fx.item(ALIEN).await;
        fx.contents
            .record(vec![(item, Decided::Unmatched(None))])
            .await
            .unwrap();
        let file = fx.contents.stored(fx.library).await.unwrap()[ALIEN].id;

        fx.contents
            .apply(fx.library, vec![Change::Remove(file)])
            .await
            .unwrap();

        assert_eq!(fx.contents.files(fx.library).await.unwrap(), []);
    }

    #[tokio::test]
    async fn undecided_leaves_out_decided_items() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        fx.store("/media/Heat (1995).mkv", 10, movie("Heat", 1995))
            .await;
        let alien = fx.item(ALIEN).await;
        let heat = fx.item("/media/Heat (1995).mkv").await;

        fx.contents
            .record(vec![(alien, Decided::Unmatched(None))])
            .await
            .unwrap();

        assert_eq!(fx.undecided().await, [heat]);
    }

    #[tokio::test]
    async fn record_skips_an_item_deleted_since_it_was_read() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let read = fx.undecided().await;
        // The file changes, which replaces its Item.
        fx.store(ALIEN, 20, movie("Alien", 1979)).await;
        let replacement = fx.item(ALIEN).await;

        let recorded = fx
            .contents
            .record(
                read.iter()
                    .map(|&id| (id, Decided::Unmatched(None)))
                    .collect::<Vec<_>>(),
            )
            .await;

        assert!(recorded.is_ok(), "{recorded:?}");
        assert_ne!(read, [replacement]);
        // The decision about the deleted Item didn't land on its replacement.
        assert_eq!(fx.undecided().await, [replacement]);
    }

    #[tokio::test]
    async fn a_pin_still_applies_after_its_items_are_replaced() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let named = fx
            .contents
            .named_like(fx.item(ALIEN).await)
            .await
            .unwrap()
            .unwrap();
        let pin = Pin::Title(ExternalId::Tmdb(949));
        fx.contents
            .pin(named.library, named.name, pin.clone(), vec![])
            .await
            .unwrap();

        fx.store(ALIEN, 20, movie("Alien", 1979)).await;

        let undecided = fx.contents.undecided(fx.library).await.unwrap();
        assert_eq!(
            undecided
                .into_iter()
                .map(|(_, _, pinned)| pinned)
                .collect::<Vec<_>>(),
            [Some(pin)]
        );
    }

    #[tokio::test]
    async fn a_pin_still_applies_after_a_reparse_gives_its_items_a_new_name() {
        let fx = Fixture::new().await;
        let heat = "/media/Heat (1995) {tmdb-949}/Heat (1995).mkv";
        let knight = "/media/A Knight of the Seven Kingdoms (2026) [tvdbid-433631]/Season 01/\
                      A Knight of the Seven Kingdoms - S01E01.mkv";
        let knight_pilot = |hint| Item::Episode {
            show: "A Knight of the Seven Kingdoms".to_owned(),
            year: Some(2026),
            hint,
            season: 1,
            episode: 1,
        };
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        fx.store(heat, 10, movie("Heat", 1995)).await;
        fx.store(knight, 10, knight_pilot(None)).await;
        let (alien_item, heat_item) = (fx.item(ALIEN).await, fx.item(heat).await);
        let knight_item = fx.item(knight).await;
        let pins = [
            (alien_item, Pin::Title(ExternalId::Tmdb(348))),
            (heat_item, Pin::Unmatched),
            (knight_item, Pin::Title(ExternalId::Tmdb(224_372))),
        ];
        for (item, pin) in pins.clone() {
            let named = fx.contents.named_like(item).await.unwrap().unwrap();
            let decided = vec![(item, Decided::Unmatched(None))];
            fx.contents
                .pin(named.library, named.name, pin, decided)
                .await
                .unwrap();
        }

        // A newer parser reads ID Hints the old one missed.
        let hinted = |title: &str, year, hint| Item::Movie {
            title: title.to_owned(),
            year: Some(year),
            hint: Some(hint),
        };
        let stored = fx.contents.stored(fx.library).await.unwrap();
        let reparse = |path: &str, item| Change::Reparse {
            file: stored[path].id,
            path: path.to_owned(),
            items: vec![item],
        };
        let changes = vec![
            reparse(
                ALIEN,
                hinted("Alien", 1979, ExternalId::Imdb("tt0078748".to_owned())),
            ),
            reparse(heat, hinted("Heat", 1995, ExternalId::Tmdb(949))),
            reparse(knight, knight_pilot(Some(ExternalId::Tvdb(433_631)))),
        ];
        fx.contents.apply(fx.library, changes).await.unwrap();

        let undecided = fx.contents.undecided(fx.library).await.unwrap();
        assert_eq!(
            undecided
                .into_iter()
                .map(|(id, item, pin)| (id, item.name().hint, pin))
                .collect::<Vec<_>>(),
            [
                (
                    alien_item,
                    Some(ExternalId::Imdb("tt0078748".to_owned())),
                    Some(pins[0].1.clone())
                ),
                (heat_item, Some(ExternalId::Tmdb(949)), Some(Pin::Unmatched)),
                (
                    knight_item,
                    Some(ExternalId::Tvdb(433_631)),
                    Some(pins[2].1.clone())
                ),
            ]
        );
    }

    #[test]
    fn a_stored_id_hint_reads_back_as_the_id_it_was() {
        let hints = [
            ExternalId::Tmdb(78),
            ExternalId::Tvdb(433_631),
            ExternalId::Imdb("tt0083658".to_owned()),
        ];
        for hint in hints {
            assert_eq!(super::parse_hint(&hint.to_string()), Some(hint));
        }
    }

    #[tokio::test]
    async fn a_reparse_adds_and_removes_items_when_their_number_changes() {
        let fx = Fixture::new().await;
        let heat = "/media/Heat (1995).mkv";
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        fx.store(heat, 10, movie("Heat", 1995)).await;
        let stored = fx.contents.stored(fx.library).await.unwrap();

        let changes = vec![
            Change::Reparse {
                file: stored[ALIEN].id,
                path: ALIEN.to_owned(),
                items: vec![movie("Alien", 1979), movie("Aliens", 1986)],
            },
            Change::Reparse {
                file: stored[heat].id,
                path: heat.to_owned(),
                items: vec![],
            },
        ];
        fx.contents.apply(fx.library, changes).await.unwrap();

        let items = fx.items(ALIEN).await;
        assert_eq!(
            items
                .iter()
                .map(|file_item| &file_item.item)
                .collect::<Vec<_>>(),
            [&movie("Alien", 1979), &movie("Aliens", 1986)]
        );
        assert_eq!(fx.items(heat).await, []);
    }

    #[tokio::test]
    async fn a_reparse_keeps_each_episode_on_the_item_with_its_numbers() {
        let fx = Fixture::new().await;
        let dark = "/media/Dark (2017)/Season 01/Dark (2017) - S01E01-E02.mkv";
        let episode = |number| Item::Episode {
            show: "Dark".to_owned(),
            year: Some(2017),
            hint: None,
            season: 1,
            episode: number,
        };
        // The older parser read only the second Episode.
        fx.store(dark, 10, episode(2)).await;
        let second = fx.item(dark).await;

        let file = fx.contents.stored(fx.library).await.unwrap()[dark].id;
        let reparse = Change::Reparse {
            file,
            path: dark.to_owned(),
            items: vec![episode(1), episode(2)],
        };
        fx.contents.apply(fx.library, vec![reparse]).await.unwrap();

        let items = fx.items(dark).await;
        let second_now = items.iter().find(|stored| stored.item == episode(2));
        assert_eq!(second_now.map(|stored| stored.id), Some(second));
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn pin_refuses_and_stores_no_decisions_when_the_library_is_gone() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let item = fx.item(ALIEN).await;
        let named = fx.contents.named_like(item).await.unwrap().unwrap();
        // An Item of another Library, which outlives the deleted one, so a decision stored
        // despite the refusal would show.
        let elsewhere = tempfile::tempdir().unwrap();
        let root = elsewhere.path().canonicalize().unwrap();
        let other = fx
            .libraries
            .create(
                "Other Movies",
                Kind::Movies,
                vec![root.to_str().unwrap().to_owned()],
            )
            .await
            .unwrap()
            .id;
        let store_heat = Change::Store {
            path: root.join("Heat (1995).mkv").to_str().unwrap().to_owned(),
            size: 10,
            modified_ns: 0,
            items: vec![movie("Heat", 1995)],
        };
        fx.contents.apply(other, vec![store_heat]).await.unwrap();
        let heat = fx.contents.undecided(other).await.unwrap()[0].0;
        fx.libraries.delete(fx.library).await.unwrap();

        let pinned = fx
            .contents
            .pin(
                named.library,
                named.name,
                Pin::Unmatched,
                vec![
                    (item, Decided::Unmatched(None)),
                    (heat, Decided::Unmatched(None)),
                ],
            )
            .await;

        assert!(matches!(pinned, Ok(None)), "{pinned:?}");
        let undecided: Vec<i64> = fx
            .contents
            .undecided(other)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect();
        assert_eq!(undecided, [heat]);
    }

    /// What TMDB says of the Movie `tmdb`, called `title` and released on `release_date`.
    fn movie_match(tmdb: i64, title: &str, release_date: &str) -> Decided {
        Decided::Matched(Box::new(Metadata {
            plugin: PluginId::from_static("tmdb"),
            ids: ExternalIds {
                tmdb: Some(tmdb),
                ..ExternalIds::default()
            },
            show_ids: ExternalIds::default(),
            title: title.to_owned(),
            original_title: None,
            show_title: None,
            overview: None,
            release_date: Some(release_date.to_owned()),
            runtime_minutes: None,
            poster: None,
            backdrop: None,
        }))
    }

    impl Fixture {
        /// Stores a file of its own for the Movie `tmdb` and Matches it to `title`, released on
        /// `release_date`.
        async fn match_movie(&self, tmdb: i64, title: &str, release_date: &str) {
            let path = format!("/media/{tmdb}.mkv");
            let year = release_date.get(..4).unwrap().parse().unwrap();
            self.store(&path, 10, movie(title, year)).await;
            let item = self.item(&path).await;
            let decided = movie_match(tmdb, title, release_date);
            self.contents.record(vec![(item, decided)]).await.unwrap();
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
            let file = Change::Store {
                path: format!("/shows/The Office - S01E0{number}.mkv"),
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
            let (item, _, _) = self.contents.undecided(library).await.unwrap().remove(0);
            self.contents.record(vec![(item, decided)]).await.unwrap();
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
        fx.match_movie(949, "Heat", "1995-12-15").await;
        let item = fx.item("/media/949.mkv").await;

        let ronin = movie_match(8195, "Ronin", "1998-09-25");
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
