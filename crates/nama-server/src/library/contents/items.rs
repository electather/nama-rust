//! What a file holds, by its name: its Items and the names they go by.

use std::borrow::Cow;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::SqliteConnection;

use crate::library::title_source::TitleKind;

use super::{Contents, ids_from_json, ids_sql, parse_hint};

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
    pub(super) kind: ItemKind,
    pub(crate) title: String,
    pub(crate) year: Option<i64>,
    pub(crate) hint: Option<ExternalId>,
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

/// Whether an Item is a Movie or an Episode, as `library_items.kind` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, sqlx::Type)]
#[sqlx(rename_all = "lowercase")]
pub(crate) enum ItemKind {
    Movie,
    Episode,
}

#[derive(Debug)]
pub(super) struct ItemRow<'a> {
    pub(super) kind: ItemKind,
    pub(super) title: Cow<'a, str>,
    pub(super) year: Option<i64>,
    /// An [`ExternalId`] as it displays, such as `tvdb-433631`.
    pub(super) hint: Option<String>,
    pub(super) season: Option<i64>,
    pub(super) episode: Option<i64>,
}

impl ItemRow<'_> {
    /// The Item the row holds. `None` for an Episode without its numbers, which the schema
    /// forbids.
    pub(super) fn into_item(self) -> Option<Item> {
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

/// A Version's first Item, by season and episode, as a subtitle or Segment source is asked about
/// it.
#[derive(Debug)]
pub(crate) struct FirstItem {
    /// The path of the Version's file.
    pub(crate) path: String,
    /// The Library the Version is in.
    pub(crate) library: i64,
    pub(crate) kind: ItemKind,
    /// For an Episode, its Season's number and its own.
    pub(crate) episode: Option<(i64, i64)>,
    /// Its Movie's ids, or for an Episode its Show's: none unless it's Matched.
    pub(crate) ids: ExternalIds,
}

impl Contents {
    /// How many Items every Library holds, for the Server's status.
    pub(crate) async fn item_count(&self) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar!(r#"SELECT COUNT(*) AS "items!: i64" FROM library_items"#)
            .fetch_one(self.db.reader())
            .await
    }
    /// The Version `version`'s [`FirstItem`], or `None` if there's no such Version or its file
    /// holds no Item.
    pub(crate) async fn first_item(&self, version: i64) -> Result<Option<FirstItem>, sqlx::Error> {
        macro_rules! select {
            ($ids:literal) => {
                sqlx::query!(
                    r#"SELECT library_files.path, library_files.library_id,
                              library_items.kind AS "kind: ItemKind",
                              library_items.season, library_items.episode, "#
                        + $ids
                        + r#"IIF(library_items.kind = 'movie', 'title', 'show')) AS "ids?: String"
                       FROM library_files
                       JOIN library_items ON library_items.file_id = library_files.id
                       LEFT JOIN item_matches ON item_matches.item_id = library_items.id
                                             AND item_matches.state = 'matched'
                       WHERE library_files.id = ?
                       ORDER BY library_items.season, library_items.episode, library_items.id
                       LIMIT 1"#,
                    version
                )
            };
        }
        let row = ids_sql!(select).fetch_optional(self.db.reader()).await?;
        Ok(row.map(|row| FirstItem {
            path: row.path,
            library: row.library_id,
            kind: row.kind,
            episode: row.season.zip(row.episode),
            ids: ids_from_json(row.ids.as_deref()),
        }))
    }

    /// The Version whose [`FirstItem`] is the Item `item`, read on `conn` so a write can act on
    /// it; `None` if it's not its file's first, or gone.
    pub(crate) async fn version_first_holding(
        conn: &mut SqliteConnection,
        item: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!(
            r#"SELECT mine.file_id AS "file_id!" FROM library_items AS mine
               WHERE mine.id = ?1
                 AND mine.id = (SELECT id FROM library_items WHERE file_id = mine.file_id
                                ORDER BY season, episode, id LIMIT 1)"#,
            item
        )
        .fetch_optional(conn)
        .await
    }
}
