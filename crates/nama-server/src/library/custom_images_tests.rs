use std::{io::Cursor, sync::Arc};

use http_body_util::{BodyExt, Full};
use hyper::{Method, body::Bytes, client::conn::http1};
use hyper_util::rt::TokioIo;
use image::{ImageFormat, RgbImage};
use nama_plugin_api::{
    PluginId,
    metadata::{ExternalId, ExternalIds},
};

use super::{CustomImages, Role, Slot, Subject, router};
use crate::{
    db::Database,
    images::ImageStore,
    library::{
        Metadata,
        fixture::{MatchedMovie, tmdb_movie},
    },
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

#[test]
fn a_slot_for_a_plugin_id_holding_a_space_or_slash_round_trips_through_its_path() {
    let id = PluginId::from("My Plugin/x".to_owned());
    let slot = Slot::plugin(&id);

    assert_eq!(slot.0, "plugin/My%20Plugin%2Fx/logo");
    assert_eq!(Slot::parse(&slot.0), Some((slot, Subject::Plugin(id))));
    assert_eq!(Slot::parse("plugin/My Plugin%2Fx/logo"), None);
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
    let custom_images = CustomImages::new(Arc::clone(&db), store);
    let images = Settings::new(db, data.path().to_owned()).images();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(custom_images.clone(), images, |_| true);
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
        "plugin/sonarr/logo",
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
        "plugin//logo",
        "plugin/sonarr/poster",
        "library/03/backdrop",
        "library/-3/backdrop",
        "library/3/poster",
        "library/tmdb-3/backdrop",
    ] {
        assert_eq!(Slot::parse(path), None, "{path}");
    }
}
