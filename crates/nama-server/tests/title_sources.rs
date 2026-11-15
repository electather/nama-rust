//! Matching through a Library's title sources, in its order, over a real socket, with the TMDB
//! Plugin enabled against a fake TMDB.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateLibraryRequest, EnablePluginRequest, ExternalId, LibraryKind,
    LibraryServiceClient, ListMoviesRequest, ListTitleSourcesRequest, PluginServiceClient,
    RefusalReason, SettingValue, UpdateLibraryRequest, UpdatePluginSettingsRequest, setting_value,
};
use refused::assert_refused;
use server::{Library, TestServer};

/// A file naming a Movie the fake TMDB finds by searching.
const ALIEN: &str = "Alien (1979)/Alien (1979).mkv";

/// The ids of the Movies `library` lists.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn movies(server: &TestServer, library: &Library) -> Vec<ExternalId> {
    let browse = BrowseServiceClient::new(HttpClient::plaintext(), server.client_config());
    let movies = browse
        .list_movies(ListMoviesRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .movies;
    movies
        .into_iter()
        .filter_map(|movie| movie.id.into_option())
        .collect()
}

fn tmdb(id: i64) -> ExternalId {
    ExternalId {
        namespace: "tmdb".to_owned(),
        value: id.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn a_library_asks_only_its_own_title_sources() {
    let server = TestServer::builder().tmdb().start().await;
    let library = server
        .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN], &["tvdb"])
        .await;

    server.scan_and_match(&library).await;
    let unasked = (movies(&server, &library).await, server.tmdb().requests());
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    libraries
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: "Media".to_owned(),
            folders: vec![library.folder().to_str().unwrap().to_owned()],
            title_sources: vec!["tmdb".to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();
    server.scan_and_match(&library).await;

    assert_eq!(unasked, (vec![], vec![]));
    assert_eq!(movies(&server, &library).await, [tmdb(348)]);
}

#[tokio::test]
async fn a_curly_tag_no_title_source_looks_up_by_is_searched_for() {
    let server = TestServer::builder().tmdb().start().await;
    let tagged = "Alien (1979) {x264-GROUP}/Alien (1979) {x264-GROUP}.mkv";

    let library = server
        .matched_library(LibraryKind::LIBRARY_KIND_MOVIES, &[tagged])
        .await;

    assert_eq!(movies(&server, &library).await, [tmdb(348)]);
}

#[tokio::test]
async fn an_order_where_no_source_searches_is_refused() {
    let server = TestServer::builder().start().await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    // fanart.tv fetches nothing until artwork is asked for, so any key does.
    let key = SettingValue {
        value: Some(setting_value::Value::TextValue("key".to_owned())),
        ..Default::default()
    };
    plugins
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "fanart".to_owned(),
            settings: std::iter::once(("api_key".to_owned(), key)).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: "fanart".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let dir = tempfile::tempdir().unwrap();
    let create = |name: &str, order: &[&str]| CreateLibraryRequest {
        name: name.to_owned(),
        kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
        folders: vec![dir.path().join(name).to_str().unwrap().to_owned()],
        title_sources: order.iter().map(|&n| n.to_owned()).collect(),
        ..Default::default()
    };
    std::fs::create_dir(dir.path().join("Fanart")).unwrap();
    std::fs::create_dir(dir.path().join("Both")).unwrap();

    let fanart_alone = libraries
        .create_library(create("Fanart", &["fanart"]))
        .await;
    let with_tmdb = libraries
        .create_library(create("Both", &["fanart", "tmdb"]))
        .await;
    let listed = libraries
        .list_title_sources(ListTitleSourcesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .title_sources;

    let refusal = assert_refused(
        fanart_alone,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TITLE_SOURCES_INVALID,
    );
    assert_eq!(refusal.field, "title_sources");
    with_tmdb.unwrap();
    let searches: Vec<_> = (listed.iter())
        .map(|source| (source.namespace.as_str(), source.searches))
        .collect();
    assert_eq!(searches, [("fanart", false)]);
}
