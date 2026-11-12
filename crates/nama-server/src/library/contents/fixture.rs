//! A Library with a Matched Movie, which in-file tests across the Server build on. It writes
//! through [`Contents`], as the crate's `AGENTS.md` asks of every test.

use std::{path::Path, sync::Arc};

use nama_plugin_api::{PluginId, metadata::ExternalIds};

use super::{Change, Contents, Decided, Item, Metadata};
use crate::{
    db::Database,
    libraries::{Kind, Libraries},
};

/// A file stored in a Library and Matched to a Movie, by the ids tests need.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MatchedMovie {
    pub(crate) library: i64,
    pub(crate) item: i64,
    /// The file's id, which is its Version's.
    pub(crate) version: i64,
}

impl MatchedMovie {
    /// Creates a Movies Library on `folder` holding one file, Matched to `matched`.
    pub(crate) async fn new(db: &Arc<Database>, folder: &Path, matched: Metadata) -> Self {
        let library = movies_library(db, folder).await;
        let path = folder
            .canonicalize()
            .unwrap()
            .join(format!("{}.mkv", matched.title));
        let contents = Contents::new(Arc::clone(db));
        Self::store(&contents, library, path.to_str().unwrap(), matched).await
    }

    /// Stores a file at `path` in `library` holding one Movie, and Matches it to `matched`.
    pub(crate) async fn store(
        contents: &Contents,
        library: i64,
        path: &str,
        matched: Metadata,
    ) -> Self {
        let file = Change::Store {
            path: path.to_owned(),
            size: 10,
            modified_ns: 0,
            items: vec![Item::Movie {
                title: matched.title.clone(),
                year: None,
                hint: None,
            }],
        };
        contents.apply(library, vec![file]).await.unwrap();
        let decided = vec![Decided::Matched(Box::new(matched))];
        let (version, items) = decide(contents, library, path, decided).await;
        Self {
            library,
            item: items[0],
            version,
        }
    }
}

/// Creates an empty Movies Library on `folder` and returns its id.
pub(crate) async fn movies_library(db: &Arc<Database>, folder: &Path) -> i64 {
    let root = folder.canonicalize().unwrap();
    Libraries::new(Arc::clone(db))
        .create(
            "Movies",
            Kind::Movies,
            vec![root.to_str().unwrap().to_owned()],
        )
        .await
        .unwrap()
        .id
}

/// Records `decided` for the Items of the file stored at `path` in `library`, in the order they
/// were stored. Returns the file's id, which is its Version's once an Item is Matched, and the
/// Items' ids.
pub(crate) async fn decide(
    contents: &Contents,
    library: i64,
    path: &str,
    decided: Vec<Decided>,
) -> (i64, Vec<i64>) {
    let version = contents.stored(library).await.unwrap()[path].id;
    let file = contents.files(library).await.unwrap();
    let file = file.into_iter().find(|file| file.path == path).unwrap();
    let items: Vec<_> = file.items.iter().map(|stored| stored.id).collect();
    assert_eq!(
        items.len(),
        decided.len(),
        "one decision per Item of {path}"
    );
    let decisions: Vec<_> = items.iter().copied().zip(decided).collect();
    contents.record(decisions).await.unwrap();
    (version, items)
}

/// What TMDB says of the Movie `tmdb`, called `title`.
pub(crate) fn tmdb_movie(tmdb: i64, title: &str) -> Metadata {
    Metadata {
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
        release_date: None,
        runtime_minutes: None,
        poster: None,
        backdrop: None,
        still: None,
    }
}
