use std::sync::Arc;

use nama_proto::nama::v1::PageRequest;

use super::{BY_TITLE, library_with_alien};
use crate::{
    browse::{Browse, Error},
    db::Database,
    library::{
        Contents, Listing, TitleSort, Watched,
        fixture::{MatchedMovie, tmdb_movie},
    },
};

#[tokio::test]
async fn a_page_token_is_refused_under_another_sort_or_filter_or_library() {
    let (data, folder) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let alien = library_with_alien(&db, &folder).await;
    let contents = Contents::new(Arc::clone(&db));
    let heat = tmdb_movie(949, "Heat");
    MatchedMovie::store(&contents, alien.library, "/media/Heat.mkv", heat).await;
    let other = tempfile::tempdir().unwrap();
    let elsewhere = MatchedMovie::new(&db, other.path(), tmdb_movie(1, "Up")).await;
    let browse = Browse::new(db);
    let by_title = BY_TITLE;
    let first = PageRequest {
        size: 1,
        ..Default::default()
    };
    let (_, paged) = (browse.movie_page(1, alien.library, &by_title, &first))
        .await
        .unwrap();
    let next = PageRequest {
        token: paged.next_token,
        ..first
    };
    let newest_first = Listing {
        sort: TitleSort::Added,
        descending: true,
        ..BY_TITLE
    };
    let not_started = Listing {
        watched: Some(Watched::NotStarted),
        ..BY_TITLE
    };
    let refused = async |library, listing| {
        let page = browse.movie_page(1, library, listing, &next).await;
        matches!(page, Err(Error::PageTokenInvalid(_)))
    };

    assert!(!refused(alien.library, &by_title).await);
    assert!(refused(alien.library, &newest_first).await);
    assert!(refused(alien.library, &not_started).await);
    assert!(refused(elsewhere.library, &by_title).await);
}
