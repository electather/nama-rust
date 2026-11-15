use std::{collections::HashSet, sync::Arc};

use nama_plugin_api::{
    PluginId,
    automation::{DeleteAsked, LibrariesAsked, LibraryKind, Permission, StoreKey},
};

use super::Automation;
use crate::{db::Database, library::contents::fixture};

#[tokio::test]
async fn a_function_without_its_permission_is_refused_in_plain_words() {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let kernel = Automation::new(db).kernel(
        PluginId::from_static("cleanup"),
        HashSet::from([Permission::PERMISSION_STORE]),
    );

    let deleted = kernel.delete_media(&DeleteAsked::default()).await;
    let listed = kernel.list_libraries(&LibrariesAsked::default()).await;

    assert_eq!(
        deleted.unwrap_err().0,
        "the Admin hasn't granted cleanup the Delete media Permission"
    );
    assert_eq!(
        listed.unwrap_err().0,
        "the Admin hasn't granted cleanup the Read Libraries Permission"
    );
    kernel.store_get(&StoreKey::default()).await.unwrap();
}

#[tokio::test]
async fn list_libraries_lists_each_library() {
    let data_dir = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data_dir.path()).await.unwrap());
    let id = fixture::movies_library(&db, media.path()).await;
    let kernel = Automation::new(db).kernel(
        PluginId::from_static("cleanup"),
        HashSet::from([Permission::PERMISSION_READ_LIBRARIES]),
    );

    let listed = kernel
        .list_libraries(&LibrariesAsked::default())
        .await
        .unwrap();

    let [library] = listed.libraries.as_slice() else {
        panic!("{listed:?}");
    };
    assert_eq!(
        (library.id, library.name.as_str(), library.kind),
        (id, "Movies", LibraryKind::LIBRARY_KIND_MOVIES.into())
    );
}
