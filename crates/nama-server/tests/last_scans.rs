//! A Library's last finished Scan, as `GetLibrary` and `ListLibraries` give it (#1125), over a
//! real socket.

#[expect(dead_code, reason = "this binary uses only some of the fixture")]
#[path = "support/server.rs"]
mod server;

use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use connectrpc::client::HttpClient;
use nama_proto::nama::v1::{
    EventType, GetLibraryRequest, LastScan, LibraryKind, LibraryServiceClient,
    ListLibrariesRequest, event,
};
use server::{TestServer, subscribe};

#[tokio::test]
async fn a_new_library_is_scanned_then_matched_without_asking() {
    let server = TestServer::builder().start().await;
    let finishes = [
        EventType::EVENT_TYPE_SCAN_FINISHED,
        EventType::EVENT_TYPE_MATCHING_FINISHED,
    ];
    let mut events = subscribe(&server.client_config(), &finishes).await;

    let library =
        (server.library_holding(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien.mkv"], &[])).await;

    let event::Event::ScanFinished(scanned) = events.next().await else {
        panic!("Matching came before the scan");
    };
    assert_eq!((scanned.library_id, scanned.added), (library.id, 1));
    let event::Event::MatchingFinished(matched) = events.next().await else {
        panic!("the scan wasn't followed by Matching");
    };
    assert_eq!(matched.library_id, library.id);
    let admin = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let got = admin
        .get_library(GetLibraryRequest {
            id: library.id,
            ..Default::default()
        })
        .await
        .unwrap()
        .into_owned();
    let last = got.library.into_option().unwrap().last_scan.into_option();
    assert_eq!(counts(&last.unwrap()), (1, 0, 0, 0));
}

#[tokio::test]
async fn each_finished_scan_replaces_the_library_s_last_scan() {
    let server = TestServer::builder().start().await;
    let library = (server.create_library(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien.mkv"])).await;
    let admin = LibraryServiceClient::new(HttpClient::plaintext(), server.client_config());
    let last_scan = async || {
        let got = admin
            .get_library(GetLibraryRequest {
                id: library.id,
                ..Default::default()
            })
            .await
            .unwrap()
            .into_owned();
        let listed = admin
            .list_libraries(ListLibrariesRequest::default())
            .await
            .unwrap()
            .into_owned();
        let last = got.library.into_option().unwrap().last_scan.into_option();
        assert_eq!(listed.libraries[0].last_scan.clone().into_option(), last);
        last.unwrap()
    };

    let seconds_now = || {
        let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        i64::try_from(since_epoch.as_secs()).unwrap()
    };
    let before = seconds_now();
    server.scan_and_match(&library).await;
    let first = last_scan().await;
    assert!((before..=seconds_now()).contains(&first.finished_at));
    assert_eq!(counts(&first), (1, 0, 0, 0));

    library.add(&["Aliens.mkv"]);
    fs::remove_file(library.path("Alien.mkv")).unwrap();
    server.scan_and_match(&library).await;
    let second = last_scan().await;
    assert!(second.finished_at >= first.finished_at);
    assert_eq!(counts(&second), (1, 0, 1, 0));
}

#[tokio::test]
async fn a_member_isn_t_given_a_library_s_last_scan() {
    let server = TestServer::builder().start().await;
    let library = (server.create_library(LibraryKind::LIBRARY_KIND_MOVIES, &["Alien.mkv"])).await;
    server.scan_and_match(&library).await;
    let member = server.user_with_default_permissions("Ripley").await;
    let listed_to = async |config| {
        let listed = LibraryServiceClient::new(HttpClient::plaintext(), config)
            .list_libraries(ListLibrariesRequest::default())
            .await
            .unwrap()
            .into_owned();
        listed.libraries[0].last_scan.is_set()
    };

    assert!(listed_to(server.client_config()).await, "an Admin sees it");
    assert!(!listed_to(member.client_config()).await);
}

/// New, changed, gone and unchanged, in that order.
fn counts(last: &LastScan) -> (u64, u64, u64, u64) {
    (last.added, last.updated, last.removed, last.unchanged)
}
