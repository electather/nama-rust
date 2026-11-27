use std::{io::Cursor, sync::Arc};

use http_body_util::{BodyExt, Full};
use hyper::{Method, body::Bytes, client::conn::http1};
use hyper_util::rt::TokioIo;
use image::{ImageFormat, RgbImage};
use nama_plugin_api::metadata::{ExternalId, ExternalIds};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};

use super::{CustomImages, Role, Slot, Subject, router};
use crate::{
    db::{self, Database},
    images::ImageStore,
    library::{
        CopiesAhead, Metadata,
        fixture::{MatchedMovie, matched_episodes_file, tmdb_movie},
        libraries::Libraries,
    },
    plugins::Registry,
    server_settings::Settings,
};

#[test]
fn a_slot_for_an_id_holding_a_slash_or_percent_round_trips_through_its_path() {
    let id = ExternalId::new("made", "a/b%c").unwrap();
    let slot = Slot::movie(&id, Role::Poster);

    assert_eq!(slot.0, "movie/made-a%2Fb%25c/poster");
    assert_eq!(Slot::parse(&slot.0), Some((slot, Subject::Movie(id))));
    let tmdb = Slot::movie(&ExternalId::tmdb(348), Role::Poster);
    assert_eq!(tmdb.0, "movie/tmdb-348/poster");
    // Only the one encoding names it.
    assert_eq!(Slot::parse("movie/made-a%2fb%25c/poster"), None);
    assert_eq!(Slot::parse("movie/made-a%2Fb%c/poster"), None);
    // As `encodeURIComponent("made-a~b(c)")` writes it.
    let kept = Slot::movie(&ExternalId::new("made", "a~b(c)").unwrap(), Role::Poster);
    assert_eq!(kept.0, "movie/made-a~b(c)/poster");
    assert_eq!(Slot::parse(&kept.0).map(|(parsed, _)| parsed), Some(kept));
}

/// Sends `body` to `path` on the server at `addr` as `method`, and returns the status.
async fn send(addr: std::net::SocketAddr, method: Method, path: &str, body: Vec<u8>) -> u16 {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (mut sender, connection) = http1::handshake(TokioIo::new(stream)).await.unwrap();
    tokio::spawn(connection);
    let request = hyper::Request::builder()
        .method(method)
        .uri(path)
        .header("host", addr.to_string())
        .body(Full::new(Bytes::from(body)))
        .unwrap();
    let response = sender.send_request(request).await.unwrap();
    let status = response.status().as_u16();
    response.into_body().collect().await.unwrap();
    status
}

#[tokio::test]
async fn an_image_for_an_id_holding_a_slash_is_uploaded_and_served_at_its_path() {
    let data = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let id = ExternalId::new("made", "a/b%c").unwrap();
    let matched = Metadata {
        ids: ExternalIds::from_iter([id.clone()]),
        ..tmdb_movie(1, "Made")
    };
    MatchedMovie::new(&db, media.path(), matched).await;
    let store = ImageStore::new(data.path(), Arc::clone(&db));
    let custom_images = CustomImages::new(Arc::clone(&db), store.clone());
    let images = Settings::new(Arc::clone(&db), data.path().to_owned()).images();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let copies_ahead =
        CopiesAhead::for_tests(&db, store.clone(), Registry::new().extension_points());
    let artwork = copies_ahead.artwork().clone();
    let app = router(custom_images.clone(), artwork, copies_ahead, images);
    tokio::spawn(async move { axum::serve(listener, app).await });
    let mut png = Cursor::new(Vec::new());
    RgbImage::new(2, 3)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    let slot = Slot::movie(&id, Role::Poster);

    let uploaded = send(
        addr,
        Method::PUT,
        &format!("/custom-images/{}", slot.0),
        png.into_inner(),
    )
    .await;

    assert_eq!(uploaded, 204);
    let url = custom_images.urls().await.unwrap().of(&slot).unwrap();
    assert_eq!(send(addr, Method::GET, &url, Vec::new()).await, 200);
}

#[test]
fn a_slot_is_named_by_one_path_only() {
    for path in [
        "movie/tmdb-348/poster",
        "movie/imdb-tt0078748/backdrop",
        "show/tvdb-81189/backdrop",
        "show/tmdb-1399/season/0/poster",
        "library/3/backdrop",
    ] {
        let slot = Slot::parse(path).map(|(slot, _)| slot);
        assert_eq!(slot, Some(Slot(path.to_owned())), "{path}");
    }
    for path in [
        "movie/Tmdb-348/poster",
        "movie/tmdb-348/logo",
        "movie/tmdb-348",
        "episode/tmdb-1/poster",
        "show/tmdb-1399/season/2/backdrop",
        "show/tmdb-1399/season/-1/poster",
        "movie/tmdb-348/poster/",
        "movie/../poster",
        "plugin/sonarr/logo",
        "library/03/backdrop",
        "library/-3/backdrop",
        "library/3/poster",
        "library/tmdb-3/backdrop",
    ] {
        assert_eq!(Slot::parse(path), None, "{path}");
    }
}

/// The version of the migration that deletes Plugins' Custom Images.
const WITHOUT_PLUGINS: i64 = 20_261_122_084_039;

#[tokio::test]
async fn upgrading_deletes_a_plugins_custom_image_and_keeps_a_titles() {
    let data = tempfile::tempdir().unwrap();
    let options = SqliteConnectOptions::new()
        .filename(data.path().join(db::FILE_NAME))
        .create_if_missing(true);
    let mut conn = SqliteConnection::connect_with(&options).await.unwrap();
    let migrations = sqlx::migrate!();
    let before = (migrations.iter().map(|migration| migration.version))
        .filter(|&version| version < WITHOUT_PLUGINS)
        .max();
    migrations.run_to(before.unwrap(), &mut conn).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO custom_images (slot, version, content_type)
         VALUES ('plugin/tmdb/logo', 'v1', 'image/svg+xml'),
                ('movie/tmdb-348/poster', 'v2', 'image/png');",
    )
    .execute(&mut conn)
    .await
    .unwrap();
    conn.close().await.unwrap();

    let db = Database::open(data.path()).await.unwrap();

    let slots: Vec<String> = sqlx::query_scalar("SELECT slot FROM custom_images")
        .fetch_all(db.reader())
        .await
        .unwrap();
    assert_eq!(slots, ["movie/tmdb-348/poster"]);
}

/// Keeps a Custom Image at each of `slots`, as a version with no bytes in the store, which the
/// slots' choice doesn't read.
async fn keep_without_bytes(db: &Database, slots: &[Slot]) {
    let slots: Vec<String> = slots.iter().map(|slot| slot.0.clone()).collect();
    db.write(move |tx| {
        Box::pin(async move {
            for slot in slots {
                sqlx::query(
                    "INSERT INTO custom_images (slot, version, content_type)
                     VALUES (?, 'v', 'image/png')",
                )
                .bind(slot)
                .execute(&mut **tx)
                .await?;
            }
            Ok::<_, sqlx::Error>(())
        })
    })
    .await
    .unwrap();
}

// Copies Ahead's walk, tested here beside `keep_without_bytes`, since only Custom Images' files
// write `custom_images`.
#[tokio::test]
async fn a_librarys_custom_images_are_its_own_and_its_titles_and_seasons_under_any_of_their_ids() {
    let data = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(data.path()).await.unwrap());
    let folders = [(); 3].map(|()| tempfile::tempdir().unwrap());
    let imdb = ExternalId::new("imdb", "tt0078748").unwrap();
    let alien = Metadata {
        ids: ExternalIds::from_iter([ExternalId::tmdb(348), imdb.clone()]),
        ..tmdb_movie(348, "Alien")
    };
    let movies = MatchedMovie::new(&db, folders[0].path(), alien)
        .await
        .library;
    let other_movie = tmdb_movie(1, "Other");
    let others = MatchedMovie::new(&db, folders[1].path(), other_movie)
        .await
        .library;
    let dark = ExternalId::tmdb(70523);
    let dark_ids = ExternalIds::from_iter([dark.clone()]);
    matched_episodes_file(&db, folders[2].path(), dark_ids, [10, 11]).await;
    let libraries = Libraries::new(Arc::clone(&db)).list().await.unwrap();
    let shows = libraries.iter().map(|library| library.id).max().unwrap();
    let movies_own = [
        Slot::movie(&imdb, Role::Poster),
        Slot::movie(&ExternalId::tmdb(348), Role::Backdrop),
        Slot::library(movies),
    ];
    let shows_own = [
        Slot::show(&dark, Role::Poster),
        Slot::show(&dark, Role::Backdrop),
        Slot::season(&dark, 1),
    ];
    let neither = [
        Slot::movie(&ExternalId::tmdb(1), Role::Poster),
        Slot::library(others),
        Slot::movie(&ExternalId::tmdb(999), Role::Poster),
        Slot::season(&dark, 2),
    ];
    keep_without_bytes(&db, &[&movies_own[..], &shows_own, &neither].concat()).await;
    let store = ImageStore::new(data.path(), Arc::clone(&db));
    let copies_ahead = CopiesAhead::for_tests(&db, store, Registry::new().extension_points());

    let of_movies = copies_ahead.custom_of(movies).await.unwrap();
    let of_shows = copies_ahead.custom_of(shows).await.unwrap();

    let slots = |of: Vec<(Slot, String)>| {
        let mut slots: Vec<String> = of.into_iter().map(|(slot, _)| slot.0).collect();
        slots.sort();
        slots
    };
    let sorted = |own: &[Slot]| {
        let mut own: Vec<String> = own.iter().map(|slot| slot.0.clone()).collect();
        own.sort();
        own
    };
    assert_eq!(slots(of_movies), sorted(&movies_own));
    assert_eq!(slots(of_shows), sorted(&shows_own));
}
