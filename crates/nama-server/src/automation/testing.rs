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
    connections,
    db::{self, Database},
    events::EventBus,
    images::ImageStore,
    library::{
        self, Artwork, Change, Contents, CustomImages, Decided, Item, Libraries, LocalImages,
        Metadata,
        fixture::{self, MatchedMovie, tmdb_movie},
    },
    playback::RunningPlans,
    plugin_settings::PluginSettings,
    plugins::Registry,
    requests::Requests,
    scanner::Scanner,
    server_settings::ServerUrl,
    users::{self, Permissions, User, Users},
};

#[path = "../../tests/support/kernel.rs"]
mod kernel_support;
pub(super) use kernel_support::ask;

pub(super) const ALIEN_TMDB_ID: i64 = 348;

/// The Automation of a Server on `db`, whose Scanner deletes and publishes nowhere.
pub(super) fn automation(db: &Arc<Database>, data: &Path) -> Arc<Automation> {
    let plugins = PluginSettings::for_tests(Arc::clone(db), Registry::new());
    let points = plugins.extension_points();
    let images = ImageStore::new(data, Arc::clone(db));
    let events = EventBus::new(Arc::clone(db), points.clone(), ServerUrl::default());
    let scanner = Scanner::new(
        Arc::clone(db),
        plugins.clone(),
        Artwork::new(images.clone(), points.clone()),
        CustomImages::new(Arc::clone(db), images.clone()),
        LocalImages::new(Arc::clone(db), images),
        events.clone(),
        None,
        RunningPlans::for_tests(db, events.clone(), data),
        Requests::new(Arc::clone(db), events, points.clone()),
    );
    let users = Users::new(Arc::clone(db), points);
    Automation::serve(Arc::clone(db), scanner, plugins, users)
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

/// Leaves a row of `plugin`'s in each table `forget_gone_plugins` deletes from, one of them the
/// User `user`'s, as if the Server last had `plugin` `days_ago`.
pub(super) async fn leave_behind(
    db: &Arc<Database>,
    plugin: &'static str,
    user: i64,
    days_ago: i64,
) {
    let id = PluginId::from_static(plugin);
    let had_at = db::now() - days_ago * 24 * 60 * 60;
    super::store::put(db, &id, None, "key".to_owned(), b"value".to_vec())
        .await
        .unwrap();
    let personal = vec![("topic".to_owned(), "mine".to_owned(), false)];
    (users::replace_personal_settings(db, user, &id, personal))
        .await
        .unwrap();
    // No owner writes these as a test needs: settings go through an enable, connections a link.
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query!(
                "INSERT INTO plugin_settings (plugin_id, enabled, settings) VALUES (?, 0, '{}')",
                plugin
            )
            .execute(&mut **tx)
            .await?;
            sqlx::query!(
                "INSERT INTO plugin_connections (user_id, plugin_id, connection, created_at)
                 VALUES (?, ?, 'chat', 0)",
                user,
                plugin
            )
            .execute(&mut **tx)
            .await?;
            sqlx::query!(
                "INSERT INTO plugins_had (plugin_id, had_at) VALUES (?, ?)
                 ON CONFLICT (plugin_id) DO UPDATE SET had_at = excluded.had_at",
                plugin,
                had_at
            )
            .execute(&mut **tx)
            .await
            .map(drop)
        })
    })
    .await
    .unwrap();
    Contents::new(Arc::clone(db)).leave_title_rows(plugin).await;
}

/// Which of the rows [`leave_behind`] leaves for `plugin` and the User `user` are still there:
/// its settings, Store entry, Personal settings, Plugin Connection, Score and Trailers, Similar
/// titles, Roles and air dates.
pub(super) async fn left_behind(db: &Arc<Database>, plugin: &'static str, user: i64) -> [bool; 8] {
    let id = PluginId::from_static(plugin);
    // Read through the grants, since a Server that never imported its config lists no settings.
    let settings = crate::plugin_settings::grants(db).await.unwrap();
    let stored = super::store::get(db, &id, None, "key").await.unwrap();
    let personal = users::personal_settings_of(db, user, &id).await.unwrap();
    let connected = connections::connected(db, &id).await.unwrap();
    let [details, similar, roles, air_dates] =
        Contents::new(Arc::clone(db)).title_rows_of(plugin).await;
    [
        settings.contains_key(plugin),
        stored.is_some(),
        !personal.is_empty(),
        !connected.is_empty(),
        details,
        similar,
        roles,
        air_dates,
    ]
}
