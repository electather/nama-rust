//! Matching through a Library's title sources, in its order, over a real socket, with the TMDB
//! Plugin enabled against a fake TMDB.

#[path = "support/refused.rs"]
mod refused;
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::sync::Arc;

use buffa::EnumValue;
use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_plugin_api::{
    Manifest, Plugin, PluginError, PluginId, PointKey, Registrar,
    metadata::{SourceManifest, TitleSource},
};
use nama_plugin_tmdb::{
    Tmdb,
    testing::{API_KEY, FakeTmdb},
};
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateLibraryRequest, DisablePluginRequest, EnablePluginRequest,
    ExternalId, LibraryKind, LibraryServiceClient, ListMoviesRequest, ListTitleSourcesRequest,
    ListUnmatchedItemsRequest, MatchServiceClient, PluginServiceClient, RefusalReason,
    SettingValue, UnmatchedReason, UpdateLibraryRequest, UpdatePluginSettingsRequest,
    setting_value,
};
use nama_server::plugins::Registry;
use refused::assert_refused;
use server::{Library, TestServer};
use tempfile::TempDir;

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
            name: library.folder().to_str().unwrap().to_owned(),
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
async fn an_order_where_no_source_searches_is_refused_whether_it_is_enabled_or_not() {
    let server = TestServer::builder().start().await;
    let dir = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let while_disabled = libraries
        .create_library(library_in(&dir, "Disabled", &["fanart"]))
        .await;
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    // fanart.tv fetches nothing until artwork is asked for, so any key does.
    let key = text("key");
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

    let fanart_alone = libraries
        .create_library(library_in(&dir, "Fanart", &["fanart"]))
        .await;
    let with_tmdb = libraries
        .create_library(library_in(&dir, "Both", &["fanart", "tmdb"]))
        .await;
    let listed = libraries
        .list_title_sources(ListTitleSourcesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .title_sources;

    for refused in [while_disabled, fanart_alone] {
        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_TITLE_SOURCES_INVALID,
        );
        assert_eq!(refusal.field, "title_sources");
    }
    with_tmdb.unwrap();
    let searches: Vec<_> = (listed.iter())
        .map(|source| (source.namespace.as_str(), source.searches))
        .collect();
    assert_eq!(searches, [("fanart", false)]);
}

#[tokio::test]
async fn an_order_naming_a_namespace_no_plugin_keeps_is_refused() {
    let server = TestServer::builder().start().await;
    let dir = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());

    let created = libraries
        .create_library(library_in(&dir, "Typo", &["zzzz", "tmdb"]))
        .await;

    assert_unknown(created, "zzzz");
}

#[tokio::test]
async fn an_order_may_name_an_installed_source_before_it_is_enabled() {
    let server = TestServer::builder().start().await;
    let dir = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());

    let created = libraries
        .create_library(library_in(&dir, "Shows", &["tvdb"]))
        .await;

    let created = created.unwrap().into_owned().library.into_option().unwrap();
    assert_eq!(created.title_sources, ["tvdb"]);
}

#[tokio::test]
async fn an_empty_order_is_refused_while_no_source_searching_keeps_the_default() {
    let plugins = Registry::new();
    plugins.describe_builtin(|| Arc::new(Declares("tmdb", None)));
    let server = TestServer::builder().plugins(plugins).start().await;
    let dir = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());

    let created = libraries
        .create_library(library_in(&dir, "Default", &[]))
        .await;

    // fanart.tv and TVDB look titles up by `tmdb`, so it is known, but no source keeping it
    // searches.
    let refusal = assert_refused(
        created,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TITLE_SOURCES_INVALID,
    );
    assert_eq!(refusal.field, "title_sources");
}

#[tokio::test]
async fn a_source_whose_plugin_is_gone_stays_in_its_library_but_no_other_may_name_it() {
    let plugins = Registry::new();
    let gone = SourceManifest {
        namespace: "gone".to_owned(),
        looks_up: Vec::new(),
        searches: true,
    };
    plugins.describe_builtin(move || Arc::new(Declares("gone", Some(gone.clone()))));
    let server = TestServer::builder().plugins(plugins).start().await;
    let library = server
        .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &[], &["gone"])
        .await;
    let server = TestServer::builder().restart(server).await;
    let dir = tempfile::tempdir().unwrap();
    let libraries = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let mut folders = vec![library.folder().to_str().unwrap().to_owned()];
    folders.extend(library_in(&dir, "Added", &[]).folders);

    let renamed = libraries
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: "Renamed".to_owned(),
            folders,
            title_sources: vec!["gone".to_owned()],
            ..Default::default()
        })
        .await;
    let other = libraries
        .create_library(library_in(&dir, "Other", &["gone", "tmdb"]))
        .await;

    let renamed = renamed.unwrap().into_owned().library.into_option().unwrap();
    assert_eq!(
        (renamed.name.as_str(), renamed.title_sources),
        ("Renamed", vec!["gone".to_owned()])
    );
    assert_unknown(other, "gone");
}

#[tokio::test]
async fn undecided_items_list_no_searching_source_until_their_source_is_enabled_again() {
    // Not `.tmdb()`: that enables TMDB in memory without a saved key, so `EnablePlugin` couldn't
    // start it again.
    let fake = FakeTmdb::start().await;
    let plugins = Registry::new();
    let endpoints = fake.endpoints();
    plugins.describe_builtin(move || Arc::new(Tmdb::with_endpoints(endpoints.clone())));
    let server = TestServer::builder().plugins(plugins).start().await;
    let plugin_service = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    plugin_service
        .update_plugin_settings(UpdatePluginSettingsRequest {
            id: "tmdb".to_owned(),
            settings: std::iter::once(("api_key".to_owned(), text(API_KEY))).collect(),
            ..Default::default()
        })
        .await
        .unwrap();
    plugin_service
        .disable_plugin(DisablePluginRequest {
            id: "tmdb".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    let library = server
        .create_library_searching(LibraryKind::LIBRARY_KIND_MOVIES, &[ALIEN], &["tmdb"])
        .await;

    server.scan_and_match(&library).await;
    let while_disabled = unmatched_reasons(&server, &library).await;
    plugin_service
        .enable_plugin(EnablePluginRequest {
            id: "tmdb".to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    server.scan_and_match(&library).await;

    assert_eq!(
        while_disabled,
        [EnumValue::from(
            UnmatchedReason::UNMATCHED_REASON_NO_SEARCHING_SOURCE
        )]
    );
    assert_eq!(
        unmatched_reasons(&server, &library).await,
        Vec::<EnumValue<UnmatchedReason>>::new()
    );
    assert_eq!(movies(&server, &library).await, [tmdb(348)]);
}

/// The reason of each item `library` lists as needing a Match.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn unmatched_reasons(
    server: &TestServer,
    library: &Library,
) -> Vec<EnumValue<UnmatchedReason>> {
    let matches = MatchServiceClient::new(HttpClient::plaintext(), server.client_config());
    let listed = matches
        .list_unmatched_items(ListUnmatchedItemsRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .items;
    listed.into_iter().map(|item| item.reason).collect()
}

fn text(value: &str) -> SettingValue {
    SettingValue {
        value: Some(setting_value::Value::TextValue(value.to_owned())),
        ..Default::default()
    }
}

/// A request creating the Movies Library `name`, in a new folder of `dir`, ordering
/// `title_sources`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
fn library_in(dir: &TempDir, name: &str, title_sources: &[&str]) -> CreateLibraryRequest {
    let folder = dir.path().join(name);
    std::fs::create_dir(&folder).unwrap();
    CreateLibraryRequest {
        name: name.to_owned(),
        kind: LibraryKind::LIBRARY_KIND_MOVIES.into(),
        folders: vec![folder.canonicalize().unwrap().to_str().unwrap().to_owned()],
        title_sources: title_sources.iter().map(|&n| n.to_owned()).collect(),
        ..Default::default()
    }
}

/// Asserts that `result` was refused for naming `namespace`, which no Plugin keeps or looks titles
/// up by.
fn assert_unknown<T>(result: Result<T, ConnectError>, namespace: &str) {
    let refusal = assert_refused(
        result,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_TITLE_SOURCES_INVALID,
    );
    assert_eq!(refusal.field, "title_sources");
    assert_eq!(
        refusal.params.get("namespace").map(String::as_str),
        Some(namespace)
    );
}

/// A Built-in Plugin with the id `.0` whose Manifest declares `.1` as its title source. It is
/// never enabled, so provides nothing.
struct Declares(&'static str, Option<SourceManifest>);

impl Plugin for Declares {
    fn manifest(&self) -> Manifest {
        Manifest {
            extension_points: vec![PointKey::provide::<dyn TitleSource>()],
            title_source: self.1.clone(),
            ..Manifest::new(PluginId::from_static(self.0))
        }
    }

    fn register(&self, _registrar: &mut Registrar) -> Result<(), PluginError> {
        Ok(())
    }
}
