//! What a Library holds: the video files a scan stored, the Items their names say they hold, what
//! Matching decided about each Item, and the Matches an admin pinned.
//!
//! [`Contents`] is the only code that reads or writes `library_files`, `library_items`,
//! `item_matches` and `match_pins`. Each of its methods is one read, or one write through the
//! database's single writer.

use std::{borrow::Cow, collections::HashMap, sync::Arc};

use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds, TitleKind},
};
use nama_proto::nama::v1::{Event, LibraryItemAdded, LibraryItemRemoved};
use sqlx::SqliteConnection;

use crate::{
    db::{Database, now},
    events::event,
};

/// Reads and writes the contents of Libraries.
#[derive(Debug, Clone)]
pub struct Contents {
    db: Arc<Database>,
}

/// Something a file holds, as far as its path says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// A Movie.
    Movie {
        /// The title, as the folder or file names it.
        title: String,
        /// The release year.
        year: Option<i64>,
        /// The Movie's TMDB id, from an ID hint.
        tmdb_id: Option<i64>,
    },
    /// One Episode of a Show.
    Episode {
        /// The Show's title, as its folder names it.
        show: String,
        /// The year the Show first aired.
        year: Option<i64>,
        /// The Show's TMDB id, from an ID hint on its folder.
        tmdb_id: Option<i64>,
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
    /// From an ID hint.
    pub(crate) tmdb_id: Option<i64>,
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
    /// Each Item's id and what it is.
    pub(crate) items: Vec<(i64, Item)>,
}

/// An Item that needs a Match, and the path of the file holding it.
#[derive(Debug)]
pub(crate) struct Unmatched {
    pub(crate) id: i64,
    pub(crate) path: String,
    pub(crate) item: Item,
}

/// A file as the last scan stored it.
#[derive(Debug)]
pub(crate) struct Stored {
    pub(crate) id: i64,
    pub(crate) size: i64,
    pub(crate) modified_ns: i64,
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
    /// Remove the stored file with this id.
    Remove(i64),
}

/// What Matching decided about one Item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Decided {
    Matched(Box<Metadata>),
    Unmatched,
}

/// The metadata stored with a Match.
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

impl Contents {
    /// The contents of the Libraries in `db`.
    #[must_use]
    pub const fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// The files stored for the Library `library`, by path.
    pub(crate) async fn stored(
        &self,
        library: i64,
    ) -> Result<HashMap<String, Stored>, sqlx::Error> {
        Ok(sqlx::query!(
            r#"SELECT id AS "id!", path, size, modified_ns FROM library_files
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
            };
            (row.path, stored)
        })
        .collect())
    }

    /// Applies `changes` to the files of the Library `library`, and returns the events for the
    /// Items that removes and adds. Replacing an Item removes what Matching decided about it.
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
                      library_items.title, library_items.year, library_items.tmdb_id,
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
                                   AND match_pins.tmdb_id IS library_items.tmdb_id
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
                    tmdb_id: row.tmdb_id,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some((row.id, item, Pin::from_row(row.pinned, pinned)))
            })
            .collect())
    }

    /// Stores what was `decided` about each Item, as of now, skipping Items that no longer exist.
    pub(crate) async fn record(&self, decided: Vec<(i64, Decided)>) -> Result<(), sqlx::Error> {
        let now = now();
        self.db
            .write(move |tx| Box::pin(insert_decisions(tx, decided, now)))
            .await
    }

    /// The Library the Item `item` is in, or `None` if there is no such Item.
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

    /// The Items of the Item `item`'s Library named like it, itself among them, or `None` if
    /// there is no such Item.
    pub(crate) async fn named_like(&self, item: i64) -> Result<Option<Named>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT files.library_id, named.id AS "id!", named.kind AS "kind: ItemKind",
                      named.title, named.year, named.tmdb_id, named.season, named.episode
               FROM library_items AS this
               JOIN library_files AS this_file ON this_file.id = this.file_id
               JOIN library_files AS files ON files.library_id = this_file.library_id
               JOIN library_items AS named ON named.file_id = files.id
               WHERE this.id = ?
                 AND named.kind = this.kind
                 AND named.title = this.title
                 AND named.year IS this.year
                 AND named.tmdb_id IS this.tmdb_id
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
                    tmdb_id: row.tmdb_id,
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
                      library_items.year, library_items.tmdb_id, library_items.season,
                      library_items.episode
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
                                   AND match_pins.tmdb_id IS library_items.tmdb_id
                                   AND match_pins.pinned_tmdb_id IS NULL
                                   AND match_pins.pinned_imdb_id IS NULL
                                   AND match_pins.pinned_tvdb_id IS NULL)
               ORDER BY library_files.path, library_items.id"#,
            library
        )
        .fetch_all(self.db.reader())
        .await?;
        Ok(rows
            .into_iter()
            .filter_map(|row| {
                let item = ItemRow {
                    kind: row.kind,
                    title: Cow::Owned(row.title),
                    year: row.year,
                    tmdb_id: row.tmdb_id,
                    season: row.season,
                    episode: row.episode,
                }
                .into_item()?;
                Some(Unmatched {
                    id: row.id,
                    path: row.path,
                    item,
                })
            })
            .collect())
    }

    /// The paths of the Library `library`'s files that hold no Items, because their names don't
    /// follow the naming conventions, in order.
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

    /// Pins `pin` for the Items of the Library `library` called `name`, replacing any pin they
    /// had, and stores what was `decided` about them, as of now, in one write. Returns `false`,
    /// having stored nothing, if the Library is gone.
    pub(crate) async fn pin(
        &self,
        library: i64,
        name: Name,
        pin: Pin,
        decided: Vec<(i64, Decided)>,
    ) -> Result<bool, sqlx::Error> {
        self.db
            .write(move |tx| Box::pin(store_pin(tx, library, name, pin, decided)))
            .await
    }
}

impl Item {
    /// The name the Item goes by.
    pub(crate) fn name(&self) -> Name {
        let row = ItemRow::from(self);
        Name {
            kind: row.kind,
            title: row.title.into_owned(),
            year: row.year,
            tmdb_id: row.tmdb_id,
        }
    }
}

impl Name {
    /// Whether the Items called this are a Movie or the Episodes of a Show.
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
        let id = ids
            .tmdb
            .map(ExternalId::Tmdb)
            .or_else(|| ids.imdb.map(ExternalId::Imdb))
            .or_else(|| ids.tvdb.map(ExternalId::Tvdb));
        Some(id.map_or(Self::Unmatched, Self::Title))
    }

    /// The pinned id, as the `pinned_*` columns of `match_pins` hold it.
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

/// Applies `changes` to the files of the Library `library`, and returns the events for the Items
/// that removes and adds.
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
                    r#"INSERT INTO library_files (library_id, path, size, modified_ns)
                       VALUES (?, ?, ?, ?)
                       ON CONFLICT (library_id, path)
                       DO UPDATE SET size = excluded.size, modified_ns = excluded.modified_ns
                       RETURNING id AS "id!""#,
                    library,
                    path,
                    size,
                    modified_ns,
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

/// The event for the Item `item`, held by the file at `path`, being added to the Library
/// `library`.
fn item_added(library: i64, item: i64, path: String) -> Event {
    event(LibraryItemAdded {
        library_id: library,
        item_id: item,
        path,
        ..Default::default()
    })
}

/// The event for the Item `item` of the Library `library` being removed.
fn item_removed(library: i64, item: i64) -> Event {
    event(LibraryItemRemoved {
        library_id: library,
        item_id: item,
        ..Default::default()
    })
}

/// Removes the Items stored for `file`, and with them what Matching decided about them, and
/// returns their ids.
async fn remove_items(conn: &mut SqliteConnection, file: i64) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar!(
        r#"DELETE FROM library_items WHERE file_id = ? RETURNING id AS "id!""#,
        file
    )
    .fetch_all(conn)
    .await
}

/// Stores `items` for `file`, and returns their ids.
async fn insert_items(
    conn: &mut SqliteConnection,
    file: i64,
    items: &[Item],
) -> Result<Vec<i64>, sqlx::Error> {
    let mut ids = Vec::with_capacity(items.len());
    for item in items {
        let row = ItemRow::from(item);
        let id = sqlx::query_scalar!(
            r#"INSERT INTO library_items (file_id, kind, title, year, tmdb_id, season, episode)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               RETURNING id AS "id!""#,
            file,
            row.kind,
            row.title,
            row.year,
            row.tmdb_id,
            row.season,
            row.episode,
        )
        .fetch_one(&mut *conn)
        .await?;
        ids.push(id);
    }
    Ok(ids)
}

/// Stores `pin` for the Items of the Library `library` called `name`, and what was `decided`
/// about them, as of now. Returns whether it stored them: it doesn't if the Library is gone.
async fn store_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: Name,
    pin: Pin,
    decided: Vec<(i64, Decided)>,
) -> Result<bool, sqlx::Error> {
    let now = now();
    if !insert_pin(conn, library, &name, &pin, now).await? {
        return Ok(false);
    }
    insert_decisions(conn, decided, now).await?;
    Ok(true)
}

/// Stores `pin` for the Items of the Library `library` called `name`, as of `now`, replacing any
/// pin they had. Returns whether it was stored: it isn't if the Library is gone.
async fn insert_pin(
    conn: &mut SqliteConnection,
    library: i64,
    name: &Name,
    pin: &Pin,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let pinned = pin.columns();
    let inserted = sqlx::query!(
        "INSERT OR REPLACE INTO match_pins (
             library_id, kind, title, year, tmdb_id,
             pinned_tmdb_id, pinned_imdb_id, pinned_tvdb_id, pinned_at)
         SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?
         WHERE EXISTS (SELECT 1 FROM libraries WHERE id = ?)",
        library,
        name.kind,
        name.title,
        name.year,
        name.tmdb_id,
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

/// Stores `decided` as of `now`, for the Items that still exist.
async fn insert_decisions(
    conn: &mut SqliteConnection,
    decided: Vec<(i64, Decided)>,
    now: i64,
) -> Result<(), sqlx::Error> {
    for (item, decision) in decided {
        let row = MatchRow::from(&decision);
        sqlx::query!(
            "INSERT OR REPLACE INTO item_matches (
                 item_id, state, plugin, tmdb_id, imdb_id, tvdb_id,
                 show_tmdb_id, show_imdb_id, show_tvdb_id, title, original_title, show_title,
                 overview, release_date, runtime_minutes, poster, backdrop, decided_at)
             SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
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
            now,
            item,
        )
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

/// The kind of an Item, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
enum ItemKind {
    Movie,
    Episode,
}

/// An Item as a row of `library_items`.
#[derive(Debug)]
struct ItemRow<'a> {
    kind: ItemKind,
    title: Cow<'a, str>,
    year: Option<i64>,
    tmdb_id: Option<i64>,
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
            tmdb_id,
            season,
            episode,
        } = self;
        Some(match kind {
            ItemKind::Movie => Item::Movie {
                title: title.into_owned(),
                year,
                tmdb_id,
            },
            ItemKind::Episode => Item::Episode {
                show: title.into_owned(),
                year,
                tmdb_id,
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
                tmdb_id,
            } => Self {
                kind: ItemKind::Movie,
                title: Cow::Borrowed(title),
                year,
                tmdb_id,
                season: None,
                episode: None,
            },
            Item::Episode {
                ref show,
                year,
                tmdb_id,
                season,
                episode,
            } => Self {
                kind: ItemKind::Episode,
                title: Cow::Borrowed(show),
                year,
                tmdb_id,
                season: Some(season),
                episode: Some(episode),
            },
        }
    }
}

/// Whether an Item is Matched, as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
enum MatchState {
    /// The Item has a Match.
    Matched,
    /// No title fit the Item confidently.
    Unmatched,
}

/// A decision as the columns of `item_matches`.
#[derive(Debug)]
struct MatchRow<'a> {
    state: MatchState,
    plugin: Option<&'a str>,
    ids: ExternalIds,
    show_ids: ExternalIds,
    metadata: MetadataColumns<'a>,
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
            Decided::Unmatched => Self {
                state: MatchState::Unmatched,
                plugin: None,
                ids: ExternalIds::default(),
                show_ids: ExternalIds::default(),
                metadata: MetadataColumns::default(),
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
            },
        }
    }
}

/// A stored file, with its Items and what Matching stored for each, as tests read them back.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// The absolute path.
    pub path: String,
    /// The size, in bytes.
    pub size: i64,
    /// What the file holds, in the order stored.
    pub items: Vec<FileItem>,
}

/// An Item of a [`File`], as tests read it back.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileItem {
    /// The Item's id.
    pub id: i64,
    /// What it is.
    pub item: Item,
    /// What Matching stored for it, or `None` while it's undecided.
    pub recorded: Option<Recorded>,
}

/// What Matching stored for an Item, as tests read it back.
#[cfg(any(test, feature = "testing"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recorded {
    /// Whether the Item is Matched, rather than Unmatched.
    pub matched: bool,
    /// The id of the Plugin whose metadata Provider made the Match.
    pub plugin: Option<String>,
    /// The Movie's or Episode's external ids.
    pub ids: ExternalIds,
    /// For an Episode, its Show's external ids.
    pub show_ids: ExternalIds,
    /// The Movie's or Episode's title.
    pub title: Option<String>,
    /// For an Episode, its Show's title.
    pub show_title: Option<String>,
    /// The poster's image path.
    pub poster: Option<String>,
}

#[cfg(any(test, feature = "testing"))]
impl Contents {
    /// The files stored for the Library `library`, by path, with their Items and what Matching
    /// stored for each.
    ///
    /// # Errors
    ///
    /// If the database can't be read.
    pub async fn files(&self, library: i64) -> Result<Vec<File>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT library_files.path, library_files.size,
                      library_items.id AS "item?", library_items.kind AS "kind?: ItemKind",
                      library_items.title AS "item_title?", library_items.year,
                      library_items.tmdb_id AS "hint", library_items.season,
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
                tmdb_id: row.hint,
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
mod tests {
    use std::sync::Arc;

    use nama_plugin_api::metadata::ExternalId;
    use tempfile::TempDir;

    use super::{Change, Contents, Decided, FileItem, Item, Pin};
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

        /// Stores the file at `path`, of `size` bytes, holding `item`.
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

        /// The Items stored for the file at `path`.
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

        /// The id of the first Item stored for the file at `path`.
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
            tmdb_id: None,
        }
    }

    #[tokio::test]
    async fn storing_a_changed_file_replaces_its_items_and_their_matches_go_with_them() {
        let fx = Fixture::new().await;
        fx.store(ALIEN, 10, movie("Alien", 1979)).await;
        let old = fx.item(ALIEN).await;
        fx.contents
            .record(vec![(old, Decided::Unmatched)])
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
            .record(vec![(item, Decided::Unmatched)])
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
            .record(vec![(alien, Decided::Unmatched)])
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
            .record(read.iter().map(|&id| (id, Decided::Unmatched)).collect())
            .await;

        assert!(matches!(recorded, Ok(())), "{recorded:?}");
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
                vec![(item, Decided::Unmatched), (heat, Decided::Unmatched)],
            )
            .await;

        assert!(matches!(pinned, Ok(false)), "{pinned:?}");
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
}
