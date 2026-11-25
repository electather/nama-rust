//! The Backups section's RPCs: listing the Backup folder and saving where Backups go and how many
//! are kept. Who may call them is in `access.rs`.

#[path = "support/refused.rs"]
mod refused;
#[path = "support/server.rs"]
#[expect(dead_code, reason = "this binary uses only some of the fixture")]
mod server;

use std::{
    fs::{self, File},
    time::{Duration, UNIX_EPOCH},
};

use connectrpc::{ErrorCode, client::HttpClient};
use nama_proto::nama::v1::{
    BackupSettings, GetBackupsRequest, GetBackupsResponse, RefusalReason, ServerServiceClient,
    UpdateBackupSettingsRequest,
};
use refused::assert_refused;
use server::TestServer;

fn client(server: &TestServer) -> ServerServiceClient<HttpClient> {
    ServerServiceClient::new(HttpClient::plaintext(), server.client_config())
}

#[expect(
    clippy::unwrap_used,
    reason = "a test helper fails its test by panicking"
)]
async fn get_backups(client: &ServerServiceClient<HttpClient>) -> GetBackupsResponse {
    (client.get_backups(GetBackupsRequest::default()))
        .await
        .unwrap()
        .into_owned()
}

fn update(folder: &str, keep: Option<u32>) -> UpdateBackupSettingsRequest {
    UpdateBackupSettingsRequest {
        settings: BackupSettings {
            folder: folder.to_owned(),
            keep,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn backups_are_listed_from_the_data_directory_by_default_with_size_and_date_newest_first() {
    let server = TestServer::builder().start().await;
    let folder = server.data_dir().join("backups");
    fs::create_dir_all(&folder).unwrap();
    for (name, bytes, modified_s) in [
        ("nama-20261120T030000Z.tar", 10, 1_000),
        ("by-hand.tar", 30, 3_000),
        ("nama-20261121T030000Z.tar", 20, 2_000),
        ("notes.txt", 1, 4_000),
    ] {
        let file = File::create(folder.join(name)).unwrap();
        file.set_len(bytes).unwrap();
        file.set_modified(UNIX_EPOCH + Duration::from_secs(modified_s))
            .unwrap();
    }

    let got = get_backups(&client(&server)).await;

    assert_eq!(got.folder, folder.display().to_string());
    let settings = got.settings.into_option().unwrap();
    assert_eq!((settings.folder.as_str(), settings.keep), ("", None));
    let listed: Vec<_> = (got.backups.iter())
        .map(|backup| {
            (
                backup.name.as_str(),
                backup.size_bytes,
                backup.modified_at_ms,
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("by-hand.tar", 30, 3_000_000),
            ("nama-20261121T030000Z.tar", 20, 2_000_000),
            ("nama-20261120T030000Z.tar", 10, 1_000_000),
        ]
    );
}

#[tokio::test]
async fn a_backup_folder_and_count_are_saved_creating_the_folder_and_read_back() {
    let server = TestServer::builder().start().await;
    let client = client(&server);
    let folder = server.data_dir().join("share/nama").display().to_string();

    let saved = client
        .update_backup_settings(update(&folder, Some(3)))
        .await;

    let saved = saved.unwrap().into_owned().settings.into_option().unwrap();
    assert_eq!(
        (saved.folder.as_str(), saved.keep),
        (folder.as_str(), Some(3))
    );
    let got = get_backups(&client).await;
    let settings = got.settings.into_option().unwrap();
    assert_eq!((settings.folder, settings.keep), (folder.clone(), Some(3)));
    assert_eq!(got.folder, folder);
    assert!(fs::metadata(&folder).unwrap().is_dir());
}

#[tokio::test]
async fn a_relative_folder_an_unwritable_one_and_keeping_none_are_refused_naming_them() {
    let server = TestServer::builder().start().await;
    let client = client(&server);
    let kept = server.data_dir().join("kept").display().to_string();
    client
        .update_backup_settings(update(&kept, Some(2)))
        .await
        .unwrap();
    let file = server.data_dir().join("a file");
    fs::write(&file, b"").unwrap();
    let unwritable = file.join("nama").display().to_string();

    for (request, reason, field) in [
        (
            update("backups", None),
            RefusalReason::REFUSAL_REASON_FOLDER_NOT_ABSOLUTE,
            "settings.folder",
        ),
        (
            update(&unwritable, None),
            RefusalReason::REFUSAL_REASON_FOLDER_NOT_WRITABLE,
            "settings.folder",
        ),
        (
            update(&kept, Some(0)),
            RefusalReason::REFUSAL_REASON_FIELD_REQUIRED,
            "settings.keep",
        ),
    ] {
        let updated = client.update_backup_settings(request).await;

        let refusal = assert_refused(updated, ErrorCode::InvalidArgument, reason);
        assert_eq!(refusal.field, field);
    }
    let settings = get_backups(&client).await.settings.into_option().unwrap();
    assert_eq!((settings.folder, settings.keep), (kept, Some(2)));
}
