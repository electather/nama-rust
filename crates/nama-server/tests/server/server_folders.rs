//! `ListServerFolders`, which lists the Server's folders for an Admin to pick one. What it leaves
//! out is checked beside it, in `src/library/folders_tests.rs`; who may call it is in `access.rs`.

use crate::support::{refused, server};

use std::fs;

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    ListServerFoldersRequest, RefusalReason, ServerFolder, ServerServiceClient,
};
use refused::assert_refused;
use server::TestServer;

fn list(path: &str) -> ListServerFoldersRequest {
    ListServerFoldersRequest {
        path: path.to_owned(),
        ..Default::default()
    }
}

#[tokio::test]
async fn the_folders_inside_a_path_are_listed_with_their_names_and_paths() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let media = server.data_dir().join("media");
    fs::create_dir_all(media.join("Movies")).unwrap();
    fs::write(media.join("notes.txt"), "").unwrap();

    let listed = client
        .list_server_folders(list(media.to_str().unwrap()))
        .await
        .unwrap()
        .into_owned();

    assert_eq!(
        listed.folders,
        [ServerFolder {
            name: "Movies".to_owned(),
            path: media.join("Movies").to_str().unwrap().to_owned(),
            ..Default::default()
        }]
    );
}

#[tokio::test]
async fn a_missing_folder_is_refused_naming_the_path() {
    let server = TestServer::builder().start().await;
    let client = ServerServiceClient::new(HttpClient::plaintext(), server.client_config());
    let missing = server.data_dir().join("gone").display().to_string();

    let listed = client.list_server_folders(list(&missing)).await;

    let refusal = assert_refused(
        listed,
        ErrorCode::InvalidArgument,
        RefusalReason::REFUSAL_REASON_FOLDER_NOT_FOUND,
    );
    assert_eq!(refusal.field, "path");
    assert_eq!(refusal.params.get("folder"), Some(&missing));
}
