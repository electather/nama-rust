use std::sync::Arc;

use nama_plugin_api::metadata::{ExternalId, ExternalIds};

use tempfile::TempDir;

use super::heaviest;
use crate::{
    browse::{Browse, Featured, tests::ada},
    db::{self, Database},
    home_rows::Suggested,
    library::{
        Contents, Libraries, Metadata,
        fixture::{MatchedMovie, movies_library, tmdb_movie},
    },
    plugins::ExtensionPoints,
};

#[tokio::test]
async fn it_never_lists_more_than_five_titles() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies_library(&db, folder.path()).await;
    let root = folder.path().canonicalize().unwrap();
    let contents = Contents::new(Arc::clone(&db));
    for tmdb in 1..=6 {
        let path = root.join(format!("{tmdb}.mkv"));
        let matched = tmdb_movie(tmdb, &format!("Movie {tmdb}"));
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }

    let featured = Browse::new(Arc::clone(&db), ExtensionPoints::default())
        .featured(ada(&db).await, &Suggested::new())
        .await
        .unwrap();

    assert_eq!(featured.len(), 5, "six added just now");
}

#[tokio::test]
async fn a_title_two_libraries_know_by_different_ids_is_listed_once() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let contents = Contents::new(Arc::clone(&db));
    let ids = ExternalIds::from_iter([ExternalId::tmdb(1), ExternalId::tvdb(9)]);
    let folders = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
    for (folder, sources) in folders.iter().zip([["tmdb", "tvdb"], ["tvdb", "tmdb"]]) {
        let library = movies_library(&db, folder.path()).await;
        let sources = sources.map(str::to_owned).to_vec();
        Libraries::new(Arc::clone(&db))
            .set_title_sources(library, sources)
            .await
            .unwrap();
        let path = folder.path().canonicalize().unwrap().join("Alien.mkv");
        let matched = Metadata {
            ids: ids.clone(),
            ..tmdb_movie(1, "Alien")
        };
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }

    let featured = Browse::new(Arc::clone(&db), ExtensionPoints::default())
        .featured(ada(&db).await, &Suggested::new())
        .await
        .unwrap();

    assert_eq!(
        featured.len(),
        1,
        "one Library knows it by TMDB's id, the other by TVDB's"
    );
}

/// A Library of the Movies of TMDB ids 1 to `count`, each added just now, and Ada's id.
async fn movies_added_now(count: i64) -> (Vec<TempDir>, Arc<Database>, i64) {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let library = movies_library(&db, folder.path()).await;
    let root = folder.path().canonicalize().unwrap();
    let contents = Contents::new(Arc::clone(&db));
    for tmdb in 1..=count {
        let path = root.join(format!("{tmdb}.mkv"));
        let matched = tmdb_movie(tmdb, &format!("Movie {tmdb}"));
        MatchedMovie::store(&contents, library, path.to_str().unwrap(), matched).await;
    }
    let ada = ada(&db).await;
    (vec![data, folder], db, ada)
}

/// The TMDB ids of the Movies of `featured`, in its order.
fn tmdb_ids(featured: &[Featured]) -> Vec<String> {
    (featured.iter())
        .map(|featured| match featured {
            Featured::Movie { movie, .. } => movie.id.to_string(),
            Featured::Show { show, .. } => show.id.to_string(),
        })
        .collect()
}

#[tokio::test]
async fn a_plugins_suggestion_at_the_top_weight_outranks_the_kernels_and_one_it_cant_find_is_dropped()
 {
    let (_dirs, db, ada) = movies_added_now(6).await;
    let suggested: Suggested = [
        ((false, ExternalId::tmdb(1)), 1.0),
        ((false, ExternalId::tmdb(900)), 1.0),
    ]
    .into();

    let featured = Browse::new(Arc::clone(&db), ExtensionPoints::default())
        .featured(ada, &suggested)
        .await
        .unwrap();

    let ids = tmdb_ids(&featured);
    assert_eq!(ids[0], "tmdb-1", "over the 0.5 of each added just now");
    assert_eq!(ids.len(), 5, "tmdb-900 is on no Library");
}

#[tokio::test]
async fn a_plugins_suggestion_at_no_weight_shows_only_with_fewer_than_five_others() {
    let (_dirs, db, ada) = movies_added_now(6).await;
    let browse = Browse::new(Arc::clone(&db), ExtensionPoints::default());
    // Past the 14 days any of them counts as newly added, so only the Plugins weigh them.
    let later = db::now() + 15 * 24 * 60 * 60;
    let at = |weights: &[(i64, f64)]| -> Suggested {
        (weights.iter())
            .map(|&(tmdb, weight)| ((false, ExternalId::tmdb(tmdb)), weight))
            .collect()
    };
    let five_others = at(&[(1, 0.0), (2, 0.1), (3, 0.1), (4, 0.1), (5, 0.1), (6, 0.1)]);
    let four_others = at(&[(1, 0.0), (2, 0.1), (3, 0.1), (4, 0.1), (5, 0.1)]);

    let with_five_others = browse.suggestions(ada, &five_others, later).await.unwrap();
    let with_four_others = browse.suggestions(ada, &four_others, later).await.unwrap();

    let ids = |suggestions| -> Vec<String> {
        let shown = heaviest(suggestions);
        (shown.iter())
            .map(|shown| shown.title.ids().next().unwrap().1.to_string())
            .collect()
    };
    assert!(!ids(with_five_others).contains(&"tmdb-1".to_owned()));
    let with_four_others = ids(with_four_others);
    assert_eq!(with_four_others.len(), 5);
    assert_eq!(with_four_others.last().map(String::as_str), Some("tmdb-1"));
}
