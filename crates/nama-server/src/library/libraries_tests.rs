use std::sync::Arc;

use nama_plugin_api::metadata::SourceManifest;
use tempfile::TempDir;

use super::{Error, Kind, Libraries, Order, check_known};
use crate::db::Database;

/// A fresh database (its folder is the first `TempDir`) and a folder a Library can hold.
async fn setup() -> (TempDir, TempDir, Libraries, String) {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let folder = media.path().canonicalize().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let folder = folder.to_str().unwrap().to_owned();
    (data, media, Libraries::new(db), folder)
}

/// Makes a folder for each of `names` in `media` and returns their paths.
fn subfolders<const N: usize>(media: &TempDir, names: [&str; N]) -> [String; N] {
    let media = media.path().canonicalize().unwrap();
    names.map(|name| {
        let path = media.join(name);
        std::fs::create_dir(&path).unwrap();
        path.to_str().unwrap().to_owned()
    })
}

fn order(namespaces: &[&str]) -> Vec<String> {
    namespaces.iter().map(|&n| n.to_owned()).collect()
}

/// An [`Order`] of `title_sources` alone, keeping the Recognisers.
fn sources(namespaces: &[&str]) -> Order {
    Order {
        title_sources: order(namespaces),
        ..Order::default()
    }
}

/// An [`Order`] setting the Recognisers `ids`, keeping the title sources.
fn recognisers(ids: &[&str]) -> Order {
    Order {
        recognisers: Some(order(ids)),
        ..Order::default()
    }
}

/// An [`Order`] for a new Library searching TMDB and asking the Recognisers `ids`.
fn tmdb_and(ids: &[&str]) -> Order {
    Order {
        recognisers: Some(order(ids)),
        ..sources(&["tmdb"])
    }
}

#[tokio::test]
async fn create_in_order_stores_and_returns_the_order() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order(
            "Shows",
            Kind::Shows,
            vec![folder],
            sources(&["tvdb", "tmdb"]),
        )
        .await
        .unwrap();
    assert_eq!(created.title_sources, order(&["tvdb", "tmdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb", "tmdb"]));
}

#[tokio::test]
async fn update_with_an_empty_order_keeps_the_old_one() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order(
            "Shows",
            Kind::Shows,
            vec![folder.clone()],
            sources(&["tvdb"]),
        )
        .await
        .unwrap();
    let updated = libraries
        .update(created.id, "Renamed", vec![folder], Order::default())
        .await
        .unwrap();
    assert_eq!(updated.title_sources, order(&["tvdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb"]));
}

#[tokio::test]
async fn update_with_a_new_order_stores_it() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create("Shows", Kind::Shows, vec![folder.clone()])
        .await
        .unwrap();
    let updated = libraries
        .update(
            created.id,
            "Shows",
            vec![folder],
            sources(&["tvdb", "tmdb"]),
        )
        .await
        .unwrap();
    assert_eq!(updated.title_sources, order(&["tvdb", "tmdb"]));
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.title_sources, order(&["tvdb", "tmdb"]));
}

#[tokio::test]
async fn a_new_library_asks_no_recognisers() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create("Shows", Kind::Shows, vec![folder])
        .await
        .unwrap();
    assert_eq!(created.recognisers, Vec::<String>::new());
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.recognisers, Vec::<String>::new());
}

#[tokio::test]
async fn create_in_order_stores_and_returns_the_recognisers() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order("Shows", Kind::Shows, vec![folder], tmdb_and(&["b", "a"]))
        .await
        .unwrap();
    assert_eq!(created.recognisers, order(&["b", "a"]));
    let listed = libraries.list().await.unwrap();
    assert_eq!(listed[0].recognisers, order(&["b", "a"]));
}

#[tokio::test]
async fn update_without_recognisers_keeps_them_and_with_none_clears_them() {
    let (_data, _media, libraries, folder) = setup().await;
    let created = libraries
        .create_in_order("Shows", Kind::Shows, vec![folder.clone()], tmdb_and(&["a"]))
        .await
        .unwrap();

    let kept = libraries
        .update(
            created.id,
            "Shows",
            vec![folder.clone()],
            sources(&["tvdb"]),
        )
        .await
        .unwrap();
    assert_eq!(kept.recognisers, order(&["a"]));
    assert_eq!(
        libraries.get(created.id).await.unwrap().recognisers,
        order(&["a"])
    );

    let cleared = libraries
        .update(created.id, "Shows", vec![folder], recognisers(&[]))
        .await
        .unwrap();
    assert_eq!(cleared.recognisers, Vec::<String>::new());
    let stored = libraries.get(created.id).await.unwrap();
    assert_eq!(stored.recognisers, Vec::<String>::new());
    assert_eq!(stored.title_sources, order(&["tvdb"]));
}

#[tokio::test]
async fn recording_the_recognisers_read_says_whether_they_changed_since_the_last_scan() {
    let (_data, _media, libraries, folder) = setup().await;
    let id = libraries
        .create("Shows", Kind::Shows, vec![folder])
        .await
        .unwrap()
        .id;
    let record = |read: &'static str| {
        (libraries.db).write(move |tx| Box::pin(Libraries::record_recognisers_read(tx, id, read)))
    };

    let none_yet = record("").await.unwrap();
    let listed = record("a,b").await.unwrap();
    let again = record("a,b").await.unwrap();
    let reordered = record("b,a").await.unwrap();

    assert_eq!(
        (none_yet, listed, again, reordered),
        (false, true, false, true)
    );
}

#[tokio::test]
async fn renaming_a_library_to_another_case_of_its_own_name_is_kept() {
    let (_data, _media, libraries, folder) = setup().await;
    let id = libraries
        .create("Movies", Kind::Movies, vec![folder.clone()])
        .await
        .unwrap()
        .id;

    let renamed = libraries
        .update(id, "MOVIES", vec![folder], Order::default())
        .await
        .unwrap();

    assert_eq!(renamed.name, "MOVIES");
}

#[tokio::test]
async fn a_library_sharing_its_name_from_before_names_were_unique_keeps_it_on_update() {
    let (_data, media, libraries, _folder) = setup().await;
    let [films, cinema, tv] = subfolders(&media, ["films", "cinema", "tv"]);
    libraries
        .create("Movies", Kind::Movies, vec![films])
        .await
        .unwrap();
    let id = libraries
        .create("Shows", Kind::Shows, vec![tv])
        .await
        .unwrap()
        .id;
    (libraries.db)
        .write(move |tx| {
            Box::pin(async move {
                sqlx::query("UPDATE libraries SET name = 'Movies' WHERE id = ?")
                    .bind(id)
                    .execute(&mut **tx)
                    .await
            })
        })
        .await
        .unwrap();

    let updated = libraries
        .update(id, "Movies", vec![cinema.clone()], Order::default())
        .await
        .unwrap();

    assert_eq!(
        (updated.name.as_str(), updated.folders),
        ("Movies", vec![cinema])
    );
}

#[tokio::test]
async fn a_name_another_library_has_with_its_accent_typed_apart_is_taken() {
    let (_data, media, libraries, _folder) = setup().await;
    let [films, tv] = subfolders(&media, ["films", "tv"]);
    libraries
        .create("Café", Kind::Movies, vec![films])
        .await
        .unwrap();

    let created = libraries.create("CAFE\u{301}", Kind::Shows, vec![tv]).await;

    assert!(
        matches!(&created, Err(Error::NameTaken { library }) if library == "Café"),
        "{created:?}"
    );
}

/// A title source keeping `namespace`, searching or not.
fn known(namespace: &str, searches: bool) -> SourceManifest {
    SourceManifest {
        namespace: namespace.to_owned(),
        looks_up: Vec::new(),
        searches,
    }
}

#[test]
fn an_order_may_drop_sources_beside_one_whose_plugin_is_gone() {
    let known = [known("fanart", false), known("tmdb", true)];
    let stored = order(&["gone", "fanart"]);

    let dropped = check_known(&order(&["gone"]), &stored, &known);
    let reordered = check_known(&order(&["fanart", "gone"]), &stored, &known);
    let left_unsearching = check_known(&order(&["fanart"]), &stored, &known);

    dropped.unwrap();
    reordered.unwrap();
    assert!(matches!(
        left_unsearching,
        Err(Error::NoTitleSourceSearches)
    ));
}

#[test]
fn a_known_stored_source_that_cannot_search_still_counts_as_not_searching() {
    let known = [known("fanart", false), known("tmdb", true)];

    let dropped_tmdb = check_known(&order(&["fanart"]), &order(&["fanart", "tmdb"]), &known);

    assert!(matches!(dropped_tmdb, Err(Error::NoTitleSourceSearches)));
}
