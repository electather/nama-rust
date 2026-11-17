//! The Fixture the unit tests of Matching share.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use nama_plugin_api::{
    metadata::{ExternalId, TMDB},
    settings::{SettingValue, Settings},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use super::Matcher;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        Artwork, Change, Contents, File, FileItem, Item, Libraries, LibraryKind, LibraryShow,
        Recorded, ScanLocks, UnmatchedReason,
    },
    plugins::{ExtensionPoints, Registry},
    server_settings::ServerUrl,
};

/// A Movies Library and a Shows Library, a Matcher, and a fake TMDB. A test stores each file
/// with the Items it states, as a scan would after reading the file's name; how a name is read
/// is the name parser's to test.
pub(super) struct Fixture {
    pub(super) matcher: Matcher,
    /// The scan locks `matcher` takes, as a scan would.
    pub(super) locks: ScanLocks,
    pub(super) contents: Contents,
    pub(super) libraries: Libraries,
    pub(super) plugins: Registry,
    pub(super) events: EventBus,
    pub(super) tmdb: FakeTmdb,
    pub(super) movies: Library,
    pub(super) shows: Library,
    _data: TempDir,
}

/// A Library whose files a test stores through [`Contents`], as a scan would.
pub(super) struct Library {
    pub(super) id: i64,
    root: PathBuf,
    contents: Contents,
}

impl Library {
    /// Stores a new file at `relative` holding `items`.
    pub(super) async fn write(&self, relative: &str, items: &[Item]) {
        self.store(relative, 5, items.to_vec()).await;
    }

    /// Stores the file at `relative` again with another size and the same Items, as a scan
    /// finding an upgraded copy would.
    pub(super) async fn replace(&self, relative: &str) {
        let path = self.path(relative);
        let files = self.contents.files(self.id).await.unwrap();
        let File { size, items, .. } = files.into_iter().find(|f| f.path == path).unwrap();
        let items = items.into_iter().map(|it| it.item).collect();
        self.store(relative, size + 1, items).await;
    }

    /// Reads the unchanged file at `relative` as holding `items`, as a scan does once a newer
    /// name parser reads it.
    pub(super) async fn reparse(&self, relative: &str, items: &[Item]) {
        let path = self.path(relative);
        let file = self.contents.stored(self.id).await.unwrap()[&path].id;
        let items = items.to_vec();
        self.apply(Change::Reparse { file, path, items }).await;
    }

    async fn store(&self, relative: &str, size: i64, items: Vec<Item>) {
        let path = self.path(relative);
        let change = Change::Store {
            path,
            size,
            modified_ns: 0,
            items,
        };
        self.apply(change).await;
    }

    async fn apply(&self, change: Change) {
        self.contents.apply(self.id, vec![change]).await.unwrap();
    }

    fn path(&self, relative: &str) -> String {
        self.root.join(relative).to_str().unwrap().to_owned()
    }

    /// `path`, relative to the Library's folder, with `/` between its parts.
    fn relative(&self, path: &str) -> String {
        PathBuf::from(path)
            .strip_prefix(&self.root)
            .unwrap()
            .components()
            .map(|part| part.as_os_str().to_str().unwrap().to_owned())
            .collect::<Vec<_>>()
            .join("/")
    }
}

/// A Movie as its file names it: by title and year, without an ID Hint.
pub(super) fn movie(title: &str, year: impl Into<Option<i64>>) -> Item {
    Item::Movie {
        title: title.to_owned(),
        year: year.into(),
        hint: None,
    }
}

/// An Episode as its file names it: by its Show's title and year, without an ID Hint.
pub(super) fn episode(show: &str, year: impl Into<Option<i64>>, season: i64, episode: i64) -> Item {
    Item::Episode {
        show: show.to_owned(),
        year: year.into(),
        hint: None,
        season,
        episode,
    }
}

/// `item` with the ID Hint `id`, as `{tmdb-949}` in its folder's name gives it.
pub(super) fn hinted(mut item: Item, id: ExternalId) -> Item {
    let (Item::Movie { hint, .. } | Item::Episode { hint, .. }) = &mut item;
    *hint = Some(id);
    item
}

impl Fixture {
    pub(super) async fn new() -> Self {
        let fixture = Self::without_tmdb().await;
        fixture.enable_tmdb();
        fixture
    }

    pub(super) async fn without_tmdb() -> Self {
        let data = tempfile::tempdir().unwrap();
        let db = Arc::new(Database::open(data.path()).await.unwrap());
        let libraries = Libraries::new(Arc::clone(&db));
        let contents = Contents::new(Arc::clone(&db));
        let create = async |name: &str, kind| {
            let root = data.path().canonicalize().unwrap().join(name);
            std::fs::create_dir(&root).unwrap();
            let folders = vec![root.to_str().unwrap().to_owned()];
            let id = libraries.create(name, kind, folders).await.unwrap().id;
            let contents = contents.clone();
            Library { id, root, contents }
        };
        let movies = create("Movies", LibraryKind::Movies).await;
        let shows = create("Shows", LibraryKind::Shows).await;
        let plugins = Registry::new();
        let events = EventBus::new(
            Arc::clone(&db),
            ExtensionPoints::default(),
            ServerUrl::default(),
        );
        let images = ImageStore::new(data.path(), Arc::clone(&db));
        let locks = ScanLocks::for_tests();
        let matcher = Matcher::new(
            db,
            plugins.extension_points(),
            Artwork::new(images, plugins.extension_points()),
            events.clone(),
            locks.clone(),
        );
        Self {
            matcher,
            locks,
            contents,
            libraries,
            plugins,
            events,
            tmdb: FakeTmdb::start().await,
            movies,
            shows,
            _data: data,
        }
    }

    pub(super) fn enable_tmdb(&self) {
        self.enable_tmdb_with_key(API_KEY);
    }

    /// Enables TMDB with a key it rejects, so every fetch fails.
    pub(super) fn enable_tmdb_rejecting(&self) {
        self.enable_tmdb_with_key("rejected");
    }

    fn enable_tmdb_with_key(&self, key: &str) {
        let settings = Settings::new(BTreeMap::from([(
            "api_key".to_owned(),
            SettingValue::Text(key.to_owned()),
        )]));
        self.plugins
            .enable_with_settings(
                Arc::new(Tmdb::with_endpoints(self.tmdb.endpoints())),
                settings,
            )
            .unwrap();
    }

    /// Matches `library` as a scan does once it has stored the files it found.
    pub(super) async fn scan(&self, library: &Library) {
        let scan = self.locks.lock(library.id).await;
        let cancel = CancellationToken::new();
        self.matcher.match_library(&scan, &cancel).await.unwrap();
    }

    pub(super) async fn items(&self, library: &Library) -> Vec<(String, FileItem)> {
        self.contents
            .files(library.id)
            .await
            .unwrap()
            .into_iter()
            .flat_map(|file| {
                let relative = library.relative(&file.path);
                file.items
                    .into_iter()
                    .map(move |item| (relative.clone(), item))
            })
            .collect()
    }

    /// What was stored for each item of `library`, as (path relative to its folder, Season
    /// and Episode, what it Matched).
    pub(super) async fn stored(
        &self,
        library: &Library,
    ) -> Vec<(String, Option<(i64, i64)>, Outcome)> {
        self.items(library)
            .await
            .into_iter()
            .map(|(relative, stored)| {
                let episode = match stored.item {
                    Item::Episode {
                        season, episode, ..
                    } => Some((season, episode)),
                    Item::Movie { .. } => None,
                };
                (relative, episode, Outcome::of(stored.recorded.as_ref()))
            })
            .collect()
    }

    pub(super) async fn outcomes(&self, library: &Library) -> Vec<Outcome> {
        self.stored(library)
            .await
            .into_iter()
            .map(|(_, _, outcome)| outcome)
            .collect()
    }

    /// Why each Item of `library` that needs a Match is Unmatched, with its file's path.
    pub(super) async fn reasons(
        &self,
        library: &Library,
    ) -> Vec<(String, Option<UnmatchedReason>)> {
        let unmatched = self.contents.unmatched(library.id).await.unwrap();
        unmatched
            .into_iter()
            .map(|item| (library.relative(&item.path), item.reason))
            .collect()
    }

    pub(super) async fn first_recorded(&self, library: &Library) -> Recorded {
        self.items(library).await.remove(0).1.recorded.unwrap()
    }

    pub(super) async fn item(&self, library: &Library, relative: &str) -> i64 {
        self.items(library)
            .await
            .into_iter()
            .find(|(path, _)| path == relative)
            .unwrap()
            .1
            .id
    }

    /// The Still of each Episode Matched in `library`, by Season and Episode.
    pub(super) async fn stills(&self, library: &Library) -> Vec<Option<String>> {
        let shows = self.contents.shows(library.id, None).await.unwrap();
        (shows.iter().flat_map(LibraryShow::episodes))
            .map(|episode| episode.still.clone())
            .collect()
    }

    pub(super) fn searches(&self) -> usize {
        self.tmdb
            .requests()
            .iter()
            .filter(|request| request.starts_with("/3/search/"))
            .count()
    }

    /// How many questions TMDB was asked about titles, leaving out the images a Match
    /// downloads in the background.
    pub(super) fn lookups(&self) -> usize {
        self.tmdb
            .requests()
            .iter()
            .filter(|request| !request.starts_with("/t/p/"))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    /// Matched to the title with this TMDB id and name.
    Matched(i64, String),
    Unmatched,
    Undecided,
}

impl Outcome {
    fn of(recorded: Option<&Recorded>) -> Self {
        match recorded {
            Some(recorded) if recorded.matched => Self::Matched(
                recorded.ids.get(TMDB).unwrap().value().parse().unwrap(),
                recorded.title.clone().unwrap(),
            ),
            Some(_) => Self::Unmatched,
            None => Self::Undecided,
        }
    }
}

pub(super) fn matched(id: i64, title: &str) -> Outcome {
    Outcome::Matched(id, title.to_owned())
}
