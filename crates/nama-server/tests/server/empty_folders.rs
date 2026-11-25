//! A Library folder that suddenly reads empty, as an unmounted share's mountpoint does, keeps its
//! items until an Admin removes them (#911), over a real socket.

use crate::support::server;

use std::fs;

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    GetLibraryRequest, LibraryKind, LibraryServiceClient, ListLibrariesRequest, ScanFinished,
    ScanLibraryRequest, SubscribeRequest, event,
};
use server::{TestServer, subscribe_with};

#[tokio::test]
async fn an_admin_removes_the_missing_items_of_a_folder_that_looks_empty() {
    let server = TestServer::builder().start().await;
    let library = (server.create_library(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien.mkv"])).await;
    let folder = library.folder().to_str().unwrap().to_owned();
    let admin = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let scan = |remove_missing_in| ScanLibraryRequest {
        id: library.id,
        remove_missing_in,
        ..Default::default()
    };
    scanned(&server, scan(None)).await;
    fs::remove_file(library.path("Alien.mkv")).unwrap();

    let held = scanned(&server, scan(None)).await;
    assert_eq!((held.removed, held.unchanged), (0, 1));
    let get = GetLibraryRequest {
        id: library.id,
        ..Default::default()
    };
    let looks_empty = async || {
        let response = admin.get_library(get.clone()).await.unwrap().into_owned();
        response.library.into_option().unwrap().empty_folders
    };
    assert_eq!(looks_empty().await, std::slice::from_ref(&folder));
    let member = server.user_with_default_permissions("Ripley").await;
    let listed = LibraryServiceClient::new(HttpClient::plaintext(), member.client_config())
        .list_libraries(ListLibrariesRequest::default())
        .await
        .unwrap()
        .into_owned();
    assert!(
        listed.libraries[0].empty_folders.is_empty(),
        "a path shown to a member"
    );

    let removed = scanned(&server, scan(Some(folder))).await;
    assert_eq!(removed.removed, 1);
    assert_eq!(looks_empty().await, Vec::<String>::new());
}

/// Sends `scan` as the Admin and waits for its `ScanFinished`, as `ScanLibrary` says to, which
/// `TestServer::scan_until` can't for a scan that removes missing items.
#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn scanned(server: &TestServer, scan: ScanLibraryRequest) -> ScanFinished {
    let about = SubscribeRequest {
        library_ids: vec![scan.id],
        ..Default::default()
    };
    let mut events = subscribe_with(&server.client_config(), about).await;
    let admin = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    admin.scan_library(scan).await.unwrap();
    while !matches!(events.next().await, event::Event::ScanStarted(_)) {}
    loop {
        if let event::Event::ScanFinished(finished) = events.next().await {
            return *finished;
        }
    }
}
