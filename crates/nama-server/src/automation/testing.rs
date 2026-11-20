//! Helpers the unit tests of the Kernel's functions share.

use std::{collections::HashSet, fs, path::Path, sync::Arc};

use buffa::MessageField;
use nama_plugin_api::{
    PluginId,
    automation::{
        DeleteAsked, DeleteStarted, Kernel, KernelRefused, Permission, TitleRef, TitleRefKind,
        TitlesAsked,
    },
    fulfiller::TitleId,
    metadata::{ExternalId, ExternalIds},
};

use super::Automation;
use crate::{
    db::Database,
    events::EventBus,
    images::ImageStore,
    library::{
        self, Artwork, Change, Contents, CustomImages, Decided, Item, Libraries, LocalImages,
        Metadata,
        fixture::{self, MatchedMovie, tmdb_movie},
    },
    playback::RunningPlans,
    plugins::{ExtensionPoints, Registry},
    requests::Requests,
    scanner::Scanner,
    server_settings::ServerUrl,
    users::{Permissions, User, Users},
};

#[path = "../../tests/support/kernel.rs"]
mod kernel_support;
pub(super) use kernel_support::ask;

pub(super) const ALIEN_TMDB_ID: i64 = 348;

/// The Automation of a Server on `db`, whose Scanner deletes and publishes nowhere.
pub(super) fn automation(db: &Arc<Database>, data: &Path) -> Arc<Automation> {
    let points = ExtensionPoints::default();
    let images = ImageStore::new(data, Arc::clone(db));
    let events = EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default());
    let scanner = Scanner::new(
        Arc::clone(db),
        crate::plugin_settings::PluginSettings::for_tests(Arc::clone(db), Registry::new()),
        Artwork::new(images.clone(), points.clone()),
        CustomImages::new(Arc::clone(db), images.clone()),
        LocalImages::new(Arc::clone(db), images),
        events.clone(),
        None,
        RunningPlans::for_tests(db, events.clone(), data),
        Requests::new(Arc::clone(db), events, points.clone()),
    );
    Automation::serve(Arc::clone(db), scanner, points)
}

/// The Kernel's functions for the cleanup Plugin on `db`, granted only `permission`, and the
/// Automation they need kept alive.
pub(super) fn kernel(
    db: &Arc<Database>,
    data: &Path,
    permission: Permission,
) -> (Arc<Automation>, Arc<dyn Kernel>) {
    let automation = automation(db, data);
    let kernel = Automation::kernel(
        &Arc::downgrade(&automation),
        PluginId::from_static("cleanup"),
        HashSet::from([permission]),
    );
    (automation, kernel)
}

/// The Movie `tmdb` of the Library `library`, as a Plugin names it.
pub(super) fn movie_ref(library: i64, tmdb: i64) -> TitleRef {
    TitleRef {
        library_id: library,
        kind: TitleRefKind::TITLE_REF_KIND_MOVIE.into(),
        id: MessageField::some(TitleId {
            namespace: "tmdb".to_owned(),
            value: tmdb.to_string(),
            ..TitleId::default()
        }),
        ..TitleRef::default()
    }
}

/// Creates a User for each of `names` on `db`, and gives their ids.
pub(super) async fn users<const N: usize>(db: &Arc<Database>, names: [&str; N]) -> [i64; N] {
    let root = User {
        id: 0,
        name: "Root".to_owned(),
        permissions: Permissions::ADMIN,
        created_at: 0,
    };
    let plugins = Registry::new();
    plugins
        .enable(Arc::new(nama_plugin_local_auth::LocalAuth))
        .unwrap();
    let users = Users::new(Arc::clone(db), plugins.extension_points());
    let mut ids = [0; N];
    for (id, name) in ids.iter_mut().zip(names) {
        *id = (users.create(&root, name, "correct horse battery staple"))
            .await
            .unwrap();
    }
    ids
}

/// The Episode `tmdb` of the Library `library`, as a Plugin names it.
pub(super) fn episode_ref(library: i64, tmdb: i64) -> TitleRef {
    TitleRef {
        kind: TitleRefKind::TITLE_REF_KIND_EPISODE.into(),
        ..movie_ref(library, tmdb)
    }
}

pub(super) const SECRETS_TMDB_ID: i64 = 1_001;
pub(super) const LIES_TMDB_ID: i64 = 1_002;
pub(super) const DARK_TMDB_ID: i64 = 70_523;

/// A Shows Library on `folder` holding one file, `Dark - S01E01-E02.mkv`, Matched to both
/// Episodes, Secrets and Lies, of the Show [`DARK_TMDB_ID`], a TV-MA Drama. Gives the Library's
/// id and the file's path.
pub(super) async fn two_episodes_in_one_file(db: &Arc<Database>, folder: &Path) -> (i64, String) {
    two_episodes_in_one_file_also_known_as(db, folder, |_tmdb| None).await
}

/// [`two_episodes_in_one_file`], with each Episode also given the id `also(its TMDB id)`.
pub(super) async fn two_episodes_in_one_file_also_known_as(
    db: &Arc<Database>,
    folder: &Path,
    also: impl Fn(i64) -> Option<ExternalId>,
) -> (i64, String) {
    let root = folder.canonicalize().unwrap();
    let library = (Libraries::new(Arc::clone(db)))
        .create(
            "Shows",
            library::LibraryKind::Shows,
            vec![root.to_str().unwrap().to_owned()],
        )
        .await
        .unwrap()
        .id;
    let path = root.join("Dark - S01E01-E02.mkv");
    fs::write(&path, "dark").unwrap();
    let path = path.to_str().unwrap().to_owned();
    let episode = |number| Item::Episode {
        show: "Dark".to_owned(),
        year: None,
        hint: None,
        season: 1,
        episode: number,
    };
    let contents = Contents::new(Arc::clone(db));
    let file = Change::Store {
        path: path.clone(),
        size: 10,
        modified_ns: 0,
        items: vec![episode(1), episode(2)],
    };
    contents.apply(library, vec![file]).await.unwrap();
    let matched = |tmdb, title| {
        let mut ids = ExternalIds::from_iter([ExternalId::tmdb(tmdb)]);
        if let Some(also) = also(tmdb) {
            ids.insert(also);
        }
        Decided::Matched(Box::new(Metadata {
            ids,
            show_ids: ExternalIds::from_iter([ExternalId::tmdb(DARK_TMDB_ID)]),
            show_title: Some("Dark".to_owned()),
            genres: vec!["Drama".to_owned()],
            content_rating: Some("TV-MA".to_owned()),
            ..tmdb_movie(tmdb, title)
        }))
    };
    let decided = vec![
        matched(SECRETS_TMDB_ID, "Secrets"),
        matched(LIES_TMDB_ID, "Lies"),
    ];
    fixture::decide(&contents, library, &path, decided).await;
    (library, path)
}

/// Creates a Movie for each of `tmdb_ids` in a Movies Library on `folder`, and gives its id.
pub(super) async fn movies(db: &Arc<Database>, folder: &Path, tmdb_ids: &[i64]) -> i64 {
    let library = fixture::movies_library(db, folder).await;
    let contents = Contents::new(Arc::clone(db));
    let root = folder.canonicalize().unwrap();
    for &tmdb in tmdb_ids {
        let path = root.join(format!("{tmdb}.mkv"));
        let matched = tmdb_movie(tmdb, &format!("Movie {tmdb}"));
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }
    library
}

/// A request no function can read: a varint cut off.
pub(super) const UNREADABLE: &[u8] = &[0xff];

pub(super) fn titles_asked(library: i64, page_size: u32, page_token: &str) -> TitlesAsked {
    TitlesAsked {
        library_id: library,
        page_size,
        page_token: page_token.to_owned(),
        ..TitlesAsked::default()
    }
}

/// What `kernel` answers deleting `title`.
pub(super) async fn delete(
    kernel: &Arc<dyn Kernel>,
    title: TitleRef,
) -> Result<DeleteStarted, KernelRefused> {
    let asked = DeleteAsked {
        title_ref: MessageField::some(title),
        ..DeleteAsked::default()
    };
    ask(kernel, "delete_media", &asked).await
}
