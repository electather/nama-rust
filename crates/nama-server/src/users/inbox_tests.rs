use nama_proto::nama::v1::{
    NotificationKind, PageRequest, Request, RequestedMovie, RequestedSeason, request::MovieOrSeason,
};
use tempfile::TempDir;

use super::{KEPT, list, mark_all_read, mark_read, store, unread};
use crate::{db::Database, page::Page};

async fn open() -> (TempDir, Database) {
    let data_dir = tempfile::tempdir().unwrap();
    let db = Database::open(data_dir.path()).await.unwrap();
    (data_dir, db)
}

async fn insert_user(db: &Database, name: &str) -> i64 {
    let name = name.to_owned();
    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query_scalar(
                "INSERT INTO users (username, display_username, permissions, created_at,
                                    updated_at)
                 VALUES (?, ?, 0, 0, 0) RETURNING id",
            )
            .bind(&name)
            .bind(&name)
            .fetch_one(&mut **tx)
            .await
        })
    })
    .await
    .unwrap()
}

fn movie(id: i64, title: &str) -> Request {
    Request {
        id,
        movie_or_season: Some(MovieOrSeason::Movie(Box::new(RequestedMovie {
            title: title.to_owned(),
            ..Default::default()
        }))),
        ..Default::default()
    }
}

/// Every notification in `user`'s inbox, newest first, page by page, as `(request id, read)`.
async fn everything(db: &Database, user: i64) -> Vec<(i64, bool)> {
    let mut asked = PageRequest {
        size: 100,
        ..Default::default()
    };
    let mut listed = Vec::new();
    loop {
        let page = Page::read(&asked, b"").unwrap();
        let (notifications, paging) = list(db, user, &page).await.unwrap();
        listed.extend(
            (notifications.iter())
                .map(|notification| (notification.request_id, notification.read_at.is_some())),
        );
        if paging.next_token.is_empty() {
            return listed;
        }
        asked.token = paging.next_token;
    }
}

#[tokio::test]
async fn a_season_is_stored_with_its_shows_title_and_number() {
    let (_dir, db) = open().await;
    let ada = insert_user(&db, "ada").await;
    let season = Request {
        id: 7,
        movie_or_season: Some(MovieOrSeason::Season(Box::new(RequestedSeason {
            show_title: "Dark".to_owned(),
            season_number: 2,
            ..Default::default()
        }))),
        ..Default::default()
    };

    store(&db, vec![ada], NotificationKind::Available, &season)
        .await
        .unwrap();

    let page = Page::read(&PageRequest::default(), b"").unwrap();
    let (listed, paging) = list(&db, ada, &page).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].kind, NotificationKind::Available);
    assert_eq!(
        (
            listed[0].title.as_str(),
            listed[0].season_number,
            listed[0].request_id
        ),
        ("Dark", Some(2), 7)
    );
    assert_eq!(paging.total_size, Some(1));
}

#[tokio::test]
async fn storing_past_the_ceiling_deletes_that_users_oldest_alone() {
    let (_dir, db) = open().await;
    let (ada, ben) = (insert_user(&db, "ada").await, insert_user(&db, "ben").await);
    store(
        &db,
        vec![ben],
        NotificationKind::Approved,
        &movie(0, "Dune"),
    )
    .await
    .unwrap();

    for request in 1..=KEPT + 2 {
        store(
            &db,
            vec![ada],
            NotificationKind::Approved,
            &movie(request, "Dune"),
        )
        .await
        .unwrap();
    }

    let kept = everything(&db, ada).await;
    assert_eq!(kept.len(), usize::try_from(KEPT).unwrap());
    assert_eq!(kept.first(), Some(&(KEPT + 2, false)));
    assert_eq!(kept.last(), Some(&(3, false)));
    assert_eq!(everything(&db, ben).await, [(0, false)]);
}

#[tokio::test]
async fn marking_read_counts_only_the_users_own_notifications() {
    let (_dir, db) = open().await;
    let (ada, ben) = (insert_user(&db, "ada").await, insert_user(&db, "ben").await);
    for request in 1..=3 {
        store(
            &db,
            vec![ada, ben],
            NotificationKind::Approved,
            &movie(request, "Dune"),
        )
        .await
        .unwrap();
    }
    let page = Page::read(&PageRequest::default(), b"").unwrap();
    let (adas, _) = list(&db, ada, &page).await.unwrap();

    assert!(!mark_read(&db, ben, adas[0].id).await.unwrap());
    assert!(mark_read(&db, ada, adas[0].id).await.unwrap());
    assert!(mark_read(&db, ada, adas[0].id).await.unwrap());
    let (ada_unread, ben_unread) = (
        unread(&db, ada).await.unwrap(),
        unread(&db, ben).await.unwrap(),
    );
    mark_all_read(&db, ada).await.unwrap();

    assert_eq!((ada_unread, ben_unread), (2, 3));
    assert_eq!(
        everything(&db, ada).await,
        [(3, true), (2, true), (1, true)]
    );
    assert_eq!(unread(&db, ben).await.unwrap(), 3);
}

#[tokio::test]
async fn deleting_a_user_deletes_their_notifications_and_a_gone_one_gets_none() {
    let (_dir, db) = open().await;
    let (ada, ben) = (insert_user(&db, "ada").await, insert_user(&db, "ben").await);
    store(
        &db,
        vec![ada, ben],
        NotificationKind::Approved,
        &movie(1, "Dune"),
    )
    .await
    .unwrap();

    db.write(move |tx| {
        Box::pin(async move {
            sqlx::query("DELETE FROM users WHERE id = ?")
                .bind(ada)
                .execute(&mut **tx)
                .await
        })
    })
    .await
    .unwrap();
    store(
        &db,
        vec![ada, ben],
        NotificationKind::Available,
        &movie(1, "Dune"),
    )
    .await
    .unwrap();

    assert_eq!(everything(&db, ada).await, Vec::new());
    assert_eq!(everything(&db, ben).await, [(1, false), (1, false)]);
}
