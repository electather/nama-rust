//! What tests of Contents share: readers of what a scan and Matching stored, and a Movies Library.

use std::{borrow::Cow, sync::Arc};

use nama_plugin_api::metadata::ExternalIds;
use tempfile::TempDir;

use super::{Change, Contents, Item, ItemKind, ItemRow, MatchState};
use crate::{
    db::Database,
    libraries::{Kind, Libraries},
    matching::METADATA_VERSION,
    scanner::PARSER_VERSION,
};

/// A stored file, with its Items and what Matching stored for each, as tests read them back.
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

    /// Marks the Matches of the Library `library` as stored by the metadata version before the
    /// current one, without the Stills that version added, as before a version bump.
    pub(crate) async fn stored_by_older_metadata(&self, library: i64) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE item_matches SET metadata_version = ? - 1, still = NULL
                         WHERE item_id IN (SELECT library_items.id FROM library_items
                                           JOIN library_files
                                             ON library_files.id = library_items.file_id
                                           WHERE library_files.library_id = ?)",
                        METADATA_VERSION,
                        library
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }

    /// Marks the Match of the item `item` as made by the Plugin `plugin`.
    pub(crate) async fn matched_by(&self, item: i64, plugin: &'static str) {
        self.db
            .write(move |tx| {
                Box::pin(async move {
                    sqlx::query!(
                        "UPDATE item_matches SET plugin = ? WHERE item_id = ?",
                        plugin,
                        item
                    )
                    .execute(&mut **tx)
                    .await
                })
            })
            .await
            .unwrap();
    }
}

pub(super) const ALIEN: &str = "/media/Alien (1979)/Alien (1979).mkv";

/// A Movies Library with nothing stored yet.
pub(super) struct Fixture {
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
    pub(super) library: i64,
    _folder: TempDir,
    _data: TempDir,
}

impl Fixture {
    pub(super) async fn new() -> Self {
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

    pub(super) async fn store(&self, path: &str, size: i64, item: Item) {
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

    pub(super) async fn items(&self, path: &str) -> Vec<FileItem> {
        self.contents
            .files(self.library)
            .await
            .unwrap()
            .into_iter()
            .find(|file| file.path == path)
            .unwrap()
            .items
    }

    pub(super) async fn item(&self, path: &str) -> i64 {
        self.items(path).await[0].id
    }

    pub(super) async fn undecided(&self) -> Vec<i64> {
        self.contents
            .undecided(self.library)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _, _)| id)
            .collect()
    }
}

pub(super) fn movie(title: &str, year: i64) -> Item {
    Item::Movie {
        title: title.to_owned(),
        year: Some(year),
        hint: None,
    }
}
