//! Recognisers over the API (ADR 0050): a scan asks an enabled Recogniser about a file the name
//! parser can't read, and Matches what it proposes as it would a parsed name.

#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;
#[path = "support/test_plugin.rs"]
mod test_plugin;

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    BrowseServiceClient, EnablePluginRequest, Episode, ExternalId, LibraryKind,
    ListEpisodesRequest, ListPluginsRequest, ListUnmatchedItemsRequest, MatchServiceClient,
    PluginServiceClient,
};
use server::{Library, TestServer, tmdb::DARK};

/// Dark's second Episode, numbered from the start of the Show, which has no `SxxEyy` for the name
/// parser to read. The test Recogniser reads it as Season 1, Episode 2.
const ABSOLUTE: &str = "Dark (2017)/Dark (2017) - 2.mkv";

/// A Server with the fake TMDB, and the test Plugin built as a Recogniser enabled if `recogniser`,
/// then a Shows Library holding [`ABSOLUTE`], scanned until Matching has finished.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn scanned(recogniser: bool) -> (TestServer, Library) {
    let server = TestServer::builder().tmdb().start().await;
    if recogniser {
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
                id: "test-plugin".to_owned(),
                ..Default::default()
            })
            .await
            .unwrap();
    }
    let library = server
        .create_library(LibraryKind::LIBRARY_KIND_SHOWS, &[ABSOLUTE])
        .await;
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
    let (server, library) = scanned(false).await;

    let unparsed = unparsed(&server, &library).await;

    assert_eq!(unparsed, [library.path(ABSOLUTE)]);
}

#[tokio::test]
async fn a_recogniser_turns_an_absolute_numbered_file_into_the_episode_the_kernel_matches() {
    let (server, library) = scanned(true).await;

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
