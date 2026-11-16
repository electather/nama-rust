//! The TVDB Installed Plugin, bundled in the Server, driven end to end through the WASM adapter
//! against a fake TVDB: Matching a Show through it, Requests fetching from it, and the ids its
//! search hits carry.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/browsing.rs"]
mod browsing;
#[expect(dead_code, reason = "this binary uploads no Custom Image through it")]
#[path = "support/custom_images.rs"]
mod custom_images;
#[expect(dead_code, reason = "this binary removes no quality profile")]
#[path = "support/fake_servarr.rs"]
mod fake_servarr;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/fake_sonarr.rs"]
mod fake_sonarr;
#[path = "support/fake_tvdb.rs"]
mod fake_tvdb;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/http.rs"]
mod http;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::client::HttpClient;
use custom_images::image;
use fake_sonarr::FakeSonarr;
use fake_tvdb::{FakeTvdb, SEVERANCE};
use hyper::{Request, body::Bytes};
use image::ImageFormat;
use nama_plugin_api::{PluginId, metadata::TitleSource};
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateSeasonRequestsRequest, ExternalId, LibraryKind, ListShowsRequest,
    RequestState, SearchShowsRequest,
};
use nama_server::plugins::Registry;
use server::{TestServer, requests::title_id};

/// A file the fake TVDB finds by searching for its Show.
const SEVERANCE_S01E01: &str = "Severance (2022)/Season 01/Severance - S01E01.mkv";

fn severance() -> ExternalId {
    title_id("tvdb", &SEVERANCE.to_string())
}

#[tokio::test]
async fn a_library_searching_tvdb_first_matches_a_show_through_tvdb() {
    let server = TestServer::builder().tmdb().start().await;
    let tvdb = FakeTvdb::start().await;
    tvdb.enable_on(&server).await;
    let library = server
        .create_library_searching(
            LibraryKind::LIBRARY_KIND_SHOWS,
            &[SEVERANCE_S01E01],
            &["tvdb", "tmdb"],
        )
        .await;

    server.scan_and_match(&library).await;

    let shows = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_shows(ListShowsRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .shows;
    let ids: Vec<_> = shows.into_iter().map(|show| show.id.unwrap()).collect();
    assert_eq!(ids, [severance()]);
    let asked = tvdb.requests();
    assert!(
        asked.contains(&format!(
            "/v4/series/{SEVERANCE}/extended?meta=translations"
        )),
        "{asked:?}"
    );
}

#[tokio::test]
async fn a_season_of_a_tvdb_show_comes_from_tvdb_and_reaches_sonarr_with_its_tvdb_id() {
    let plugins = Registry::new();
    plugins.set_provider_precedence::<dyn TitleSource>(vec![
        PluginId::from_static("tmdb"),
        PluginId::from_static("tvdb"),
    ]);
    let server = TestServer::builder().plugins(plugins).tmdb().start().await;
    let tvdb = FakeTvdb::start().await;
    tvdb.enable_on(&server).await;
    let sonarr = FakeSonarr::start().await;
    sonarr.enable_on(&server).await;
    let ada = server.user_with_default_permissions("Ada").await;

    let asked = server
        .requests(&ada)
        .create_season_requests(CreateSeasonRequestsRequest {
            show_id: severance().into(),
            season_numbers: vec![1],
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .requests;
    let [pending] = asked.as_slice() else {
        panic!("asked {} Requests, not one", asked.len());
    };
    let approved = server.approve(pending.id).await;
    sonarr.finish_refreshes().await;

    assert_eq!(approved.state, RequestState::Processing);
    let added: Vec<_> = sonarr
        .shows()
        .iter()
        .map(|show| show["tvdbId"].clone())
        .collect();
    assert_eq!(added, [serde_json::json!(SEVERANCE)]);
    let season = format!("/v4/series/{SEVERANCE}/episodes/default?season=1");
    assert!(tvdb.requests().contains(&season), "{:?}", tvdb.requests());
    let from_tmdb = server.tmdb().requests();
    assert!(
        !from_tmdb.iter().any(|path| path.contains("/tv/")),
        "{from_tmdb:?}"
    );
}

/// TMDB's search hits carry one id each, so only a TVDB hit, with its IMDb and TMDB ids, proves a
/// Custom Image kept under any of a hit's ids shows on it.
#[tokio::test]
async fn a_custom_poster_kept_under_another_id_of_a_tvdb_search_hit_shows_on_the_hit() {
    let server = TestServer::builder().start().await;
    let tvdb = FakeTvdb::start().await;
    tvdb.enable_on(&server).await;
    let path = "/custom-images/show/imdb-tt11280740/poster";
    let body = http_body_util::Full::new(Bytes::from(image(ImageFormat::Png, 2, 3)));
    let upload = Request::put(path).body(body).unwrap();
    let uploaded = http::send(&server, upload, Some(&server.admin_token)).await;
    assert_eq!(uploaded.status, 204);

    let shows = server
        .admin_requests()
        .search_shows(SearchShowsRequest {
            title: "Severance".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .shows;

    let hit = shows.iter().find(|it| *it.id == severance()).unwrap();
    let poster = hit.poster_url.clone().unwrap();
    assert!(poster.starts_with(&format!("/media{path}/")), "{poster}");
}
