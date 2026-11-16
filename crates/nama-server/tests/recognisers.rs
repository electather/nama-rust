//! Recognisers over the API (ADR 0050): a Library lists the Recognisers it asks, in an order the
//! Admin sets, and a scan asks those about a file the name parser can't read, and Matches what
//! they propose as it would a parsed name.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use connectrpc::{ConnectError, ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    BrowseServiceClient, CreateLibraryRequest, EnablePluginRequest, Episode, ExtensionPoint,
    ExternalId, GetLibraryRequest, LibraryKind, LibraryServiceClient, ListEpisodesRequest,
    ListLibrariesRequest, ListPluginsRequest, ListUnmatchedItemsRequest, MatchServiceClient,
    PluginServiceClient, RecogniserOrder, RefusalReason, UpdateLibraryRequest,
    UpdateLibraryResponse,
};
use refused::assert_refused;
use server::{Library, TestServer, tmdb::DARK};

/// Dark's second Episode, numbered from the start of the Show, which has no `SxxEyy` for the name
/// parser to read. The test Recogniser reads it as Season 1, Episode 2.
const ABSOLUTE: &str = "Dark (2017)/Dark (2017) - 2.mkv";

/// The id of the test Plugin built as a Recogniser.
const RECOGNISER: &str = "test-plugin";

/// A Server with the fake TMDB and the test Plugin built as a Recogniser, enabled.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn with_recogniser() -> TestServer {
    let server = TestServer::builder().tmdb().start().await;
    let dir = server.data_dir().join("plugins");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("test.wasm"), test_plugin::build("recogniser")).unwrap();
    let plugins = PluginServiceClient::new(HttpClient::plaintext(), server.client_config());
    // Listing is what reads `plugins/`.
    plugins
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap();
    plugins
        .enable_plugin(EnablePluginRequest {
            id: RECOGNISER.to_owned(),
            ..Default::default()
        })
        .await
        .unwrap();
    server
}

fn libraries(server: &TestServer) -> LibraryServiceClient<HttpClient> {
    LibraryServiceClient::new(HttpClient::plaintext(), server.client_config())
}

fn ids(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|&id| id.to_owned()).collect()
}

/// Updates `library` as the Admin, keeping its name and folder, with `order` as its
/// `recogniser_order`.
async fn update_recognisers(
    server: &TestServer,
    library: &Library,
    order: Option<&[&str]>,
) -> Result<UpdateLibraryResponse, ConnectError> {
    libraries(server)
        .update_library(UpdateLibraryRequest {
            id: library.id,
            name: "Media".to_owned(),
            folders: vec![library.folder().to_string_lossy().into_owned()],
            recogniser_order: order
                .map(|order| RecogniserOrder {
                    ids: ids(order),
                    ..Default::default()
                })
                .into(),
            ..Default::default()
        })
        .await
        .map(connectrpc::UnaryResponse::into_owned)
}

/// The Recognisers `library` lists, as `GetLibrary` sends them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn listed(server: &TestServer, library: &Library) -> Vec<String> {
    libraries(server)
        .get_library(GetLibraryRequest {
            id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap()
        .recogniser_ids
}

/// Where the test Recogniser is when a Library is scanned.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Recogniser {
    /// Not installed.
    Absent,
    /// Enabled, but the Library doesn't list it.
    Unlisted,
    /// Enabled and listed by the Library.
    Listed,
}

/// A Server with the fake TMDB and the test Recogniser as `recogniser` says, then a Shows Library
/// holding [`ABSOLUTE`], scanned until Matching has finished.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn scanned(recogniser: Recogniser) -> (TestServer, Library) {
    let server = if recogniser == Recogniser::Absent {
        TestServer::builder().tmdb().start().await
    } else {
        with_recogniser().await
    };
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[ABSOLUTE])
        .await;
    if recogniser == Recogniser::Listed {
        update_recognisers(&server, &library, Some(&[RECOGNISER]))
            .await
            .unwrap();
    }
    server.scan_and_match(&library).await;
    (server, library)
}

/// The paths of the Library's files that hold no Items, as `ListUnmatchedItems` lists them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn unparsed(server: &TestServer, library: &Library) -> Vec<String> {
    MatchServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_unmatched_items(ListUnmatchedItemsRequest {
            library_id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .unparsed_paths
}

/// Dark's Season 1 Episodes in the Library, as `ListEpisodes` lists them.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn dark_season_1(server: &TestServer, library: &Library) -> Vec<Episode> {
    BrowseServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_episodes(ListEpisodesRequest {
            library_id: library.id,
            show_id: ExternalId {
                namespace: "tmdb".to_owned(),
                value: DARK.to_string(),
                ..Default::default()
            }
            .into(),
            season_number: 1,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .episodes
}

#[tokio::test]
async fn without_a_recogniser_an_absolute_numbered_file_stays_unparsed() {
    let (server, library) = scanned(Recogniser::Absent).await;

    let unparsed = unparsed(&server, &library).await;

    assert_eq!(unparsed, [library.path(ABSOLUTE)]);
}

#[tokio::test]
async fn a_library_not_listing_an_enabled_recogniser_leaves_the_file_unparsed() {
    let (server, library) = scanned(Recogniser::Unlisted).await;

    let unparsed = unparsed(&server, &library).await;

    assert_eq!(unparsed, [library.path(ABSOLUTE)]);
}

#[tokio::test]
async fn a_recogniser_turns_an_absolute_numbered_file_into_the_episode_the_kernel_matches() {
    let (server, library) = scanned(Recogniser::Listed).await;

    let unparsed = unparsed(&server, &library).await;
    let episodes = dark_season_1(&server, &library).await;

    assert_eq!(unparsed, Vec::<String>::new());
    let holding: Vec<u32> = episodes
        .iter()
        .filter(|episode| {
            (episode.versions.iter()).any(|version| version.file_name == "Dark (2017) - 2.mkv")
        })
        .map(|episode| episode.episode_number)
        .collect();
    assert_eq!(holding, [2]);
}

#[tokio::test]
async fn a_library_created_with_recognisers_lists_them() {
    let server = with_recogniser().await;
    let folder = tempfile::tempdir().unwrap();

    let created = libraries(&server)
        .create_library(CreateLibraryRequest {
            name: "Anime".to_owned(),
            kind: LibraryKind::LIBRARY_KIND_SHOWS.into(),
            folders: vec![folder.path().to_string_lossy().into_owned()],
            recogniser_ids: ids(&[RECOGNISER]),
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned()
        .library
        .into_option()
        .unwrap();

    assert_eq!(created.recogniser_ids, [RECOGNISER]);
    let listed = libraries(&server)
        .list_libraries(ListLibrariesRequest::default())
        .await
        .unwrap()
        .into_owned()
        .libraries;
    assert_eq!(listed[0].recogniser_ids, [RECOGNISER]);
}

#[tokio::test]
async fn a_new_library_lists_no_recognisers() {
    let server = with_recogniser().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;

    assert_eq!(listed(&server, &library).await, Vec::<String>::new());
}

#[tokio::test]
async fn an_update_sets_the_recognisers_and_an_empty_order_clears_them() {
    let server = with_recogniser().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;

    let set = update_recognisers(&server, &library, Some(&[RECOGNISER]))
        .await
        .unwrap();
    assert_eq!(
        set.library.into_option().unwrap().recogniser_ids,
        [RECOGNISER]
    );
    assert_eq!(listed(&server, &library).await, [RECOGNISER]);

    update_recognisers(&server, &library, Some(&[]))
        .await
        .unwrap();
    assert_eq!(listed(&server, &library).await, Vec::<String>::new());
}

#[tokio::test]
async fn an_update_without_a_recogniser_order_keeps_it() {
    let server = with_recogniser().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;
    update_recognisers(&server, &library, Some(&[RECOGNISER]))
        .await
        .unwrap();

    let kept = update_recognisers(&server, &library, None).await.unwrap();

    assert_eq!(
        kept.library.into_option().unwrap().recogniser_ids,
        [RECOGNISER]
    );
    assert_eq!(listed(&server, &library).await, [RECOGNISER]);
}

/// Creates a Library with `order` as its `recogniser_ids`, and updates `library` with it as its
/// `recogniser_order`, and asserts both are refused naming the field and `refused_id`.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn assert_order_refused(
    server: &TestServer,
    library: &Library,
    order: &[&str],
    refused_id: &str,
) {
    let folder = tempfile::tempdir().unwrap();
    let created = libraries(server)
        .create_library(CreateLibraryRequest {
            name: "Anime".to_owned(),
            kind: LibraryKind::LIBRARY_KIND_SHOWS.into(),
            folders: vec![folder.path().to_string_lossy().into_owned()],
            recogniser_ids: ids(order),
            ..Default::default()
        })
        .await;
    let updated = update_recognisers(server, library, Some(order)).await;

    for (refused, field) in [
        (created.map(drop), "recogniser_ids"),
        (updated.map(drop), "recogniser_order"),
    ] {
        let refusal = assert_refused(
            refused,
            ErrorCode::InvalidArgument,
            RefusalReason::REFUSAL_REASON_RECOGNISERS_INVALID,
        );
        assert_eq!(refusal.field, field, "{order:?}");
        assert_eq!(
            refusal.params.get("pluginId").map(String::as_str),
            Some(refused_id),
            "{order:?}"
        );
    }
}

#[tokio::test]
async fn a_recogniser_no_plugin_serves_is_refused_naming_the_field_and_id() {
    let server = with_recogniser().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;
    // `local-auth` is a Plugin, but no Recogniser; `nope` is no Plugin at all.
    for unknown in ["nope", "local-auth"] {
        assert_order_refused(&server, &library, &[RECOGNISER, unknown], unknown).await;
    }
    assert_eq!(listed(&server, &library).await, Vec::<String>::new());
}

#[tokio::test]
async fn a_recogniser_named_twice_is_refused_naming_the_field_and_id() {
    let server = with_recogniser().await;
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[])
        .await;

    assert_order_refused(&server, &library, &[RECOGNISER, RECOGNISER], RECOGNISER).await;

    assert_eq!(listed(&server, &library).await, Vec::<String>::new());
}

#[tokio::test]
async fn list_plugins_says_which_plugins_are_recognisers() {
    let server = with_recogniser().await;

    let listed = PluginServiceClient::new(HttpClient::plaintext(), server.client_config())
        .list_plugins(ListPluginsRequest::default())
        .await
        .unwrap()
        .into_owned();

    let recognisers: Vec<&str> = (listed.summaries.iter())
        .filter(|plugin| {
            (plugin.extension_points.iter())
                .any(|point| *point == ExtensionPoint::EXTENSION_POINT_RECOGNISER)
        })
        .map(|plugin| plugin.id.as_str())
        .collect();
    assert_eq!(recognisers, [RECOGNISER]);
}
