use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    sync::Arc,
};

use nama_plugin_api::PluginId;
use tempfile::TempDir;

use super::{Error, restore, write};
use crate::{
    db::{self, Database},
    server_settings::Settings,
};

const IMAGE: &str = "custom-images/movie/tmdb-1/poster/abc";

/// A Server's files: its config file, its data directory, and where its backup goes.
struct Server {
    dir: TempDir,
    config: PathBuf,
    data_dir: PathBuf,
    backup: PathBuf,
}

impl Server {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let config = dir.path().join("nama.toml");
        fs::write(&config, "name = \"Backed up\"\n").unwrap();
        let data_dir = dir.path().join("data");
        let image = data_dir.join(IMAGE);
        fs::create_dir_all(image.parent().unwrap()).unwrap();
        fs::write(&image, b"poster").unwrap();
        Self {
            config,
            backup: dir.path().join("backup.tar"),
            data_dir,
            dir,
        }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.data_dir.join(name)
    }
}

async fn put(db: &Database, key: &str) {
    let key = key.to_owned();
    db.write(|tx| {
        Box::pin(async move {
            sqlx::query!("INSERT INTO schema_meta (key, value) VALUES (?, 'x')", key)
                .execute(&mut **tx)
                .await?;
            Ok::<_, sqlx::Error>(())
        })
    })
    .await
    .unwrap();
}

async fn keys_in(file: &Path) -> Vec<String> {
    let mut conn = db::connect_existing(file).await.unwrap();
    sqlx::query_scalar!("SELECT key FROM schema_meta WHERE value = 'x' ORDER BY key")
        .fetch_all(&mut conn)
        .await
        .unwrap()
}

fn entries(tar: &Path) -> Vec<String> {
    tar::Archive::new(File::open(tar).unwrap())
        .entries()
        .unwrap()
        .map(|entry| entry.unwrap().path().unwrap().display().to_string())
        .collect()
}

fn manifest(highest_migration: i64) -> String {
    format!(r#"{{"server_version":"0.0.0","highest_migration":{highest_migration}}}"#)
}

fn append_file(tar: &mut tar::Builder<File>, name: &str, contents: &str) {
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o644);
    tar.append_data(&mut header, name, contents.as_bytes())
        .unwrap();
}

/// A Server backed up while its database was open, with what changed after: a new row, its
/// Custom Image deleted and its config edited. Its database is closed again.
async fn changed_after_its_backup() -> Server {
    let server = Server::new();
    let db = Database::open(&server.data_dir).await.unwrap();
    put(&db, "backed-up").await;
    write(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();
    put(&db, "after").await;
    db.close().await;
    fs::remove_file(server.path(IMAGE)).unwrap();
    fs::write(&server.config, "name = \"Changed\"\n").unwrap();
    server
}

#[tokio::test]
async fn a_backup_of_an_open_database_restores_its_data_images_and_config() {
    let server = changed_after_its_backup().await;

    restore(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();

    let db = Database::open(&server.data_dir).await.unwrap();
    db.close().await;
    assert_eq!(keys_in(&server.path("nama.db")).await, ["backed-up"]);
    assert_eq!(fs::read(server.path(IMAGE)).unwrap(), b"poster");
    assert_eq!(
        fs::read_to_string(&server.config).unwrap(),
        "name = \"Backed up\"\n"
    );
}

#[tokio::test]
async fn a_restore_keeps_what_it_replaced_beside_it_as_before_restore() {
    let server = changed_after_its_backup().await;

    restore(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();

    assert_eq!(
        keys_in(&server.path("nama.db.before-restore")).await,
        ["after", "backed-up"]
    );
    assert!(server.path("custom-images.before-restore").is_dir());
    assert_eq!(
        fs::read_to_string(server.dir.path().join("nama.toml.before-restore")).unwrap(),
        "name = \"Changed\"\n"
    );
}

#[tokio::test]
async fn a_backup_holds_its_manifest_first_and_no_caches() {
    let server = Server::new();
    let db = Database::open(&server.data_dir).await.unwrap();
    fs::create_dir_all(server.path("artwork-cache")).unwrap();

    write(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();
    db.close().await;

    let entries = entries(&server.backup);
    assert_eq!(entries[..3], ["manifest.json", "nama.db", "nama.toml"]);
    assert!(entries.iter().any(|entry| entry == IMAGE), "{entries:?}");
    assert!(
        !entries
            .iter()
            .any(|entry| entry.starts_with("artwork-cache")),
        "{entries:?}"
    );
}

#[tokio::test]
async fn a_backup_leaves_custom_images_out_when_a_plugin_stores_images() {
    let server = Server::new();
    let db = Arc::new(Database::open(&server.data_dir).await.unwrap());
    Settings::new(Arc::clone(&db), server.path("segments"))
        .set_image_store(Some(PluginId::from("nama-plugin-s3".to_owned())))
        .await
        .unwrap();

    write(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();

    let entries = entries(&server.backup);
    assert!(
        !entries
            .iter()
            .any(|entry| entry.starts_with("custom-images")),
        "{entries:?}"
    );
}

#[tokio::test]
async fn a_restore_refuses_a_backup_from_a_newer_server_and_changes_nothing() {
    let server = changed_after_its_backup().await;
    let newer = db::highest_migration_known() + 1;
    let mut tar = tar::Builder::new(File::create(&server.backup).unwrap());
    append_file(&mut tar, "manifest.json", &manifest(newer));
    append_file(&mut tar, "nama.db", "not a database");
    tar.finish().unwrap();
    let before = fs::read_dir(&server.data_dir).unwrap().count();

    let refused = restore(&server.data_dir, &server.config, &server.backup).await;

    assert!(
        matches!(refused, Err(Error::Newer { migration }) if migration == newer),
        "{refused:?}"
    );
    assert_eq!(fs::read_dir(&server.data_dir).unwrap().count(), before);
    assert_eq!(
        keys_in(&server.path("nama.db")).await,
        ["after", "backed-up"]
    );
}

#[tokio::test]
async fn a_restore_refuses_while_the_server_has_its_database_open() {
    let server = changed_after_its_backup().await;
    let db = Database::open(&server.data_dir).await.unwrap();

    let refused = restore(&server.data_dir, &server.config, &server.backup).await;

    assert!(matches!(refused, Err(Error::Running)), "{refused:?}");
    db.close().await;
    assert!(!server.path("nama.db.before-restore").exists());
}

#[tokio::test]
async fn a_restore_replaces_a_database_too_broken_to_read() {
    let server = changed_after_its_backup().await;
    fs::write(server.path("nama.db"), b"not a database").unwrap();

    restore(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();

    assert_eq!(keys_in(&server.path("nama.db")).await, ["backed-up"]);
}

#[tokio::test]
async fn a_restore_deletes_a_wal_left_without_its_database() {
    let server = changed_after_its_backup().await;
    fs::remove_file(server.path("nama.db")).unwrap();
    fs::write(server.path("nama.db-wal"), b"stale").unwrap();

    restore(&server.data_dir, &server.config, &server.backup)
        .await
        .unwrap();

    assert!(!server.path("nama.db-wal").exists());
    assert_eq!(keys_in(&server.path("nama.db")).await, ["backed-up"]);
}

#[tokio::test]
async fn a_restore_refuses_a_backup_holding_a_symlink_and_changes_nothing() {
    let server = changed_after_its_backup().await;
    let mut tar = tar::Builder::new(File::create(&server.backup).unwrap());
    append_file(
        &mut tar,
        "manifest.json",
        &manifest(db::highest_migration_known()),
    );
    let mut link = tar::Header::new_gnu();
    link.set_entry_type(tar::EntryType::Symlink);
    link.set_size(0);
    tar.append_link(&mut link, "nama.toml", "/etc/passwd")
        .unwrap();
    append_file(&mut tar, "nama.db", "not a database");
    tar.finish().unwrap();

    let refused = restore(&server.data_dir, &server.config, &server.backup).await;

    assert!(matches!(refused, Err(Error::NotABackup(_))), "{refused:?}");
    assert_eq!(
        fs::read_to_string(&server.config).unwrap(),
        "name = \"Changed\"\n"
    );
    assert!(!server.path("nama.db.before-restore").exists());
}
