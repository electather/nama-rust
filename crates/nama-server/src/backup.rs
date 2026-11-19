//! Backups of a Server (ADR 0068): one uncompressed `.tar` holding a copy of its database, its
//! config file, its Custom Images while the image store is local disk, and a manifest saying what
//! wrote it. Caches stay out, since the Server makes them again.

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sqlx::Connection as _;
use tempfile::TempDir;

use crate::{db, images::CUSTOM_IMAGES_DIR, server_settings};

/// The backup's first entry, so a restore can refuse it before unpacking the rest.
const MANIFEST: &str = "manifest.json";

/// The config file's name inside a backup, whatever it was called on the Server.
const CONFIG: &str = "nama.toml";

/// Why a backup or a restore failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Reading or writing a file failed.
    #[error("{0}")]
    Io(#[from] io::Error),
    /// SQLite couldn't open, copy or read a database.
    #[error("{0}")]
    Sqlx(#[from] sqlx::Error),
    /// The file isn't a backup a Server wrote; holds what's wrong with it.
    #[error("not a Nama backup: {0}")]
    NotABackup(String),
    /// A newer version of Nama wrote the backup, so nothing was restored.
    #[error(
        "the backup is from a newer version of Nama (migration {migration} is unknown to this \
         one), so nothing was restored: restore it with that version"
    )]
    Newer {
        /// The newest migration the backup's database has had.
        migration: i64,
    },
    /// A Server has the database open, so nothing was restored.
    #[error("the Server is running, so nothing was restored: stop it first")]
    Running,
}

/// What wrote a backup, which a restore checks before changing anything.
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    server_version: String,
    highest_migration: i64,
}

/// Writes a backup of the Server whose data directory is `data_dir` and whose config file, if
/// there is one, is `config_file`, to the `.tar` at `to`, replacing any file there. Safe while
/// the Server serves: the database is copied through a connection of its own.
///
/// # Errors
///
/// If `data_dir` has no database, or a file can't be read or written. A failed backup leaves the
/// file at `to` as it was.
pub async fn write(data_dir: &Path, config_file: &Path, to: &Path) -> Result<(), Error> {
    // Beside `to`, so the finished `.tar` is renamed into place, never copied.
    let staging = TempDir::new_in(to.parent().unwrap_or_else(|| Path::new("")))?;
    let db_copy = staging.path().join(db::FILE_NAME);
    let images_are_local = copy_database(data_dir, &db_copy, staging.path()).await?;
    let mut parts = vec![
        (MANIFEST, staging.path().join(MANIFEST)),
        (db::FILE_NAME, db_copy),
    ];
    if config_file.is_file() {
        parts.push((CONFIG, config_file.to_owned()));
    }
    let images = data_dir.join(CUSTOM_IMAGES_DIR);
    // shortcut: an image removed while this runs fails the backup, or leaves the database naming
    // it; copy images through `ImageStore`'s locks if scheduled backups hit that.
    if images_are_local && images.is_dir() {
        parts.push((CUSTOM_IMAGES_DIR, images));
    }
    let to = to.to_owned();
    tokio::task::spawn_blocking(move || {
        let tar = staging.path().join("backup.tar");
        write_tar(&parts, &tar)?;
        fs::rename(tar, to)
    })
    .await
    .map_err(io::Error::other)??;
    Ok(())
}

/// Copies the database in `data_dir` to `copy` and writes its manifest into `staging`. Returns
/// whether the copy keeps its images on local disk: read from the copy, so it matches its rows.
async fn copy_database(data_dir: &Path, copy: &Path, staging: &Path) -> Result<bool, Error> {
    let mut live = db::connect_existing(&data_dir.join(db::FILE_NAME)).await?;
    let copied = db::copy_to(&mut live, copy).await;
    let closed = live.close().await;
    copied?;
    closed?;
    let mut snapshot = db::connect_existing(copy).await?;
    let manifest = Manifest {
        server_version: env!("CARGO_PKG_VERSION").to_owned(),
        highest_migration: db::highest_migration_applied(&mut snapshot)
            .await?
            .unwrap_or_default(),
    };
    let images_are_local = server_settings::stored_image_store(&mut snapshot)
        .await?
        .is_none();
    snapshot.close().await?;
    tokio::fs::write(
        staging.join(MANIFEST),
        serde_json::to_vec(&manifest).map_err(io::Error::from)?,
    )
    .await?;
    Ok(images_are_local)
}

/// Writes `parts`, each a name in the `.tar` and the file or folder it comes from, in order, to
/// `tar`, synced so the rename after it can't replace a good backup with one a power cut loses.
fn write_tar(parts: &[(&str, PathBuf)], tar: &Path) -> io::Result<()> {
    let mut builder = tar::Builder::new(File::create(tar)?);
    for (name, path) in parts {
        if path.is_dir() {
            builder.append_dir_all(name, path)?;
        } else {
            builder.append_path_with_name(path, name)?;
        }
    }
    builder.into_inner()?.sync_all()
}

/// Restores the backup at `from` into the stopped Server whose data directory is `data_dir` and
/// whose config file is `config_file`. Each of the database, the config file and `custom-images`
/// the backup holds replaces the Server's, which is kept beside it with `.before-restore` added
/// to its name, replacing what the last restore kept.
///
/// # Errors
///
/// [`Error::Running`] while a Server has the database open, [`Error::Newer`] for a backup a newer
/// Server wrote, and [`Error::NotABackup`] for another file, each before anything is replaced.
/// If a file can't be moved, what was already replaced stays replaced.
pub async fn restore(data_dir: &Path, config_file: &Path, from: &Path) -> Result<(), Error> {
    let db_file = data_dir.join(db::FILE_NAME);
    if db_file.try_exists()? {
        refuse_if_open(&db_file).await?;
    }
    let (data_dir, config_file, from) =
        (data_dir.to_owned(), config_file.to_owned(), from.to_owned());
    tokio::task::spawn_blocking(move || {
        fs::create_dir_all(&data_dir)?;
        // In the data directory, so the database and images are renamed into place.
        let staging = TempDir::new_in(&data_dir)?;
        unpack(&from, staging.path())?;
        swap_in(staging.path(), &data_dir, &config_file)
    })
    .await
    .map_err(io::Error::other)?
}

/// Fails with [`Error::Running`] while another connection has the database at `file` open.
/// SQLite deletes the WAL when the last connection closes, after moving what a crash left in it
/// into the database, so a WAL still there after this one closes belongs to a running Server.
async fn refuse_if_open(file: &Path) -> Result<(), Error> {
    let mut conn = db::connect_existing(file).await?;
    // A read, without which SQLite never opens the WAL and so never cleans it up on close. Its
    // error is ignored: a database too broken to read is the kind a restore replaces.
    let _: Result<_, _> = db::highest_migration_applied(&mut conn).await;
    conn.close().await?;
    if with_suffix(file, "-wal").try_exists()? {
        return Err(Error::Running);
    }
    Ok(())
}

/// Unpacks the backup at `from` into `into`, after checking its manifest.
fn unpack(from: &Path, into: &Path) -> Result<(), Error> {
    let mut archive = tar::Archive::new(File::open(from)?);
    let mut entries = archive.entries()?;
    let mut first = entries
        .next()
        .transpose()?
        .ok_or_else(|| Error::NotABackup("it's empty".to_owned()))?;
    if *first.path()? != *Path::new(MANIFEST) {
        return Err(Error::NotABackup(format!(
            "it doesn't start with {MANIFEST}"
        )));
    }
    let manifest: Manifest = serde_json::from_reader(&mut first)
        .map_err(|err| Error::NotABackup(format!("its {MANIFEST} can't be read: {err}")))?;
    if manifest.highest_migration > db::highest_migration_known() {
        return Err(Error::Newer {
            migration: manifest.highest_migration,
        });
    }
    for entry in entries {
        // `unpack_in` skips an entry naming a path outside `into`.
        entry?.unpack_in(into)?;
    }
    if !into.join(db::FILE_NAME).is_file() {
        return Err(Error::NotABackup(format!("it holds no {}", db::FILE_NAME)));
    }
    Ok(())
}

/// Puts what `staged` holds in place of the Server's own, keeping each replaced one beside it.
/// The database goes last, so a config file or folder that can't be moved stops the restore
/// before it.
fn swap_in(staged: &Path, data_dir: &Path, config_file: &Path) -> Result<(), Error> {
    let config = staged.join(CONFIG);
    if config.is_file() {
        move_aside(config_file)?;
        // Copied: the config file may be on another filesystem than the data directory.
        fs::copy(config, config_file)?;
    }
    let images = staged.join(CUSTOM_IMAGES_DIR);
    if images.is_dir() {
        let current = data_dir.join(CUSTOM_IMAGES_DIR);
        move_aside(&current)?;
        fs::rename(images, current)?;
    }
    let db_file = data_dir.join(db::FILE_NAME);
    move_aside(&db_file)?;
    // Only stale ones can be left now, and SQLite would replay a stale WAL into the new database.
    for leftover in ["-wal", "-shm"] {
        match fs::remove_file(with_suffix(&db_file, leftover)) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err.into()),
            _ => {}
        }
    }
    fs::rename(staged.join(db::FILE_NAME), db_file)?;
    Ok(())
}

/// Renames the file or folder at `path`, if there is one, to `<path>.before-restore`, replacing
/// what was there.
fn move_aside(path: &Path) -> io::Result<()> {
    if !path.try_exists()? {
        return Ok(());
    }
    let aside = with_suffix(path, ".before-restore");
    match fs::symlink_metadata(&aside) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(&aside)?,
        Ok(_) => fs::remove_file(&aside)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    fs::rename(path, aside)
}

/// `path` with `suffix` added to its last part.
fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.as_os_str().to_owned();
    path.push(suffix);
    path.into()
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
