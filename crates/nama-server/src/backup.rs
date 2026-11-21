//! Backups of a Server (ADR 0068): one uncompressed `.tar` holding a copy of its database, its
//! config file, its Custom Images while the image store is local disk, and a manifest saying what
//! wrote it. Caches stay out, since the Server makes them again. `Backups` are those the Server
//! writes by itself.

use std::{
    fs::{self, File},
    io,
    num::NonZeroU32,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use jiff::{Timestamp, civil::DateTime};
use nama_plugin_api::automation::BackedUp;
use serde::{Deserialize, Serialize};
use sqlx::Connection as _;
use tempfile::TempDir;

use crate::{
    db,
    images::CUSTOM_IMAGES_DIR,
    server_settings::{self, BackupValues, Settings},
};

/// The backup's first entry, so a restore can refuse it before unpacking the rest.
const MANIFEST: &str = "manifest.json";

/// The config file's name inside a backup, whatever it was called on the Server.
const CONFIG: &str = "nama.toml";

/// The folder in the data directory the Server's own Backups go to while no other is set.
const DEFAULT_FOLDER: &str = "backups";

/// How many of the Server's own Backups are kept while no count is set.
const DEFAULT_KEPT: u32 = 7;

/// The UTC time a Backup the Server wrote is named by, `nama-<time>Z.tar`: fixed width, so its
/// names sort by time, and without `:`, which Windows and SMB shares refuse in a name.
const NAME_TIME: &str = "%Y%m%dT%H%M%S";

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

/// The Backups the Server writes by itself into the folder the Server settings name, keeping the
/// newest of them. Clones share the settings.
#[derive(Debug, Clone)]
pub(crate) struct Backups {
    data_dir: PathBuf,
    config_file: PathBuf,
    settings: Settings,
}

/// A `.tar` file in the Backup folder.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) name: String,
    /// Its size in bytes.
    pub(crate) size: u64,
    /// When it was last written, in milliseconds since the Unix epoch.
    pub(crate) modified_ms: i64,
}

impl Backups {
    /// The Backups of the Server whose data directory is `data_dir` and whose config file, if
    /// there is one, is `config_file`, written where `settings` say.
    pub(crate) fn new(data_dir: &Path, config_file: &Path, settings: Settings) -> Self {
        Self {
            data_dir: data_dir.to_owned(),
            config_file: config_file.to_owned(),
            settings,
        }
    }

    /// Writes a Backup named `nama-<UTC time>Z.tar` into the Backup folder, creating it if it's
    /// missing, then deletes the oldest files so named there beyond the count kept, never the one
    /// it wrote. Answers as the Kernel function `back_up` does.
    ///
    /// # Errors
    ///
    /// If the Backup can't be written, which deletes nothing and leaves no file under its name,
    /// or an old one can't be deleted.
    pub(crate) async fn write(&self) -> Result<BackedUp, Error> {
        let settings = self.settings.now().backups;
        let folder = self.folder_of(&settings);
        let time = Timestamp::from_second(db::now()).map_err(io::Error::other)?;
        let name = format!("nama-{}Z.tar", time.strftime(NAME_TIME));
        let path = folder.join(&name);
        tokio::fs::create_dir_all(&folder).await?;
        write(&self.data_dir, &self.config_file, &path).await?;
        let size = tokio::fs::metadata(&path).await?.len();
        let kept = settings.kept.map_or(DEFAULT_KEPT, NonZeroU32::get);
        let written = name.clone();
        tokio::task::spawn_blocking(move || delete_oldest(&folder, &written, kept))
            .await
            .map_err(io::Error::other)??;
        Ok(BackedUp {
            name,
            size_bytes: size,
            ..BackedUp::default()
        })
    }

    /// The Backup folder in use now, and every `.tar` file in it, newest first, or none while
    /// it's missing. The folder is read once, so the files are always that folder's.
    ///
    /// # Errors
    ///
    /// If the folder can't be read.
    pub(crate) async fn list(&self) -> io::Result<(PathBuf, Vec<Listed>)> {
        let folder = self.folder_of(&self.settings.now().backups);
        let listed_in = folder.clone();
        let listed = tokio::task::spawn_blocking(move || list(&listed_in))
            .await
            .map_err(io::Error::other)??;
        Ok((folder, listed))
    }

    fn folder_of(&self, settings: &BackupValues) -> PathBuf {
        if settings.folder.is_empty() {
            return self.data_dir.join(DEFAULT_FOLDER);
        }
        PathBuf::from(&settings.folder)
    }
}

/// Every `.tar` file in `folder`, newest first, or none if there's no `folder`.
fn list(folder: &Path) -> io::Result<Vec<Listed>> {
    let entries = match fs::read_dir(folder) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        entries => entries?,
    };
    let mut listed = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry
            .file_name()
            .into_string()
            .ok()
            .filter(|name| is_tar(name))
        else {
            continue;
        };
        let meta = entry.metadata()?;
        if !meta.is_file() {
            continue;
        }
        let modified = meta
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        listed.push(Listed {
            name,
            size: meta.len(),
            modified_ms: i64::try_from(modified.as_millis()).unwrap_or(i64::MAX),
        });
    }
    listed.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then(b.name.cmp(&a.name)));
    Ok(listed)
}

/// Whether `name` is a `.tar` file's, whatever its case.
fn is_tar(name: &str) -> bool {
    (Path::new(name).extension()).is_some_and(|extension| extension.eq_ignore_ascii_case("tar"))
}

/// Deletes the oldest of the Backups the Server wrote in `folder` beyond the newest `kept`, counting
/// the one just `written` as the newest whatever its name: a Backup named for a later time, after
/// the clock went back, must not get it deleted.
fn delete_oldest(folder: &Path, written: &str, kept: u32) -> io::Result<()> {
    let mut own = Vec::new();
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| name != written && is_own(name))
            && entry.file_type()?.is_file()
        {
            own.push(name);
        }
    }
    own.sort_unstable_by(|a, b| b.cmp(a));
    let older_kept = usize::try_from(kept.saturating_sub(1)).unwrap_or(usize::MAX);
    for name in own.iter().skip(older_kept) {
        fs::remove_file(folder.join(name))?;
    }
    Ok(())
}

/// Whether `name` is one [`Backups::write`] gives, so a file the Admin put beside them, such as a
/// Backup `nama-server backup` wrote under another name, is never deleted.
fn is_own(name: &str) -> bool {
    (name.strip_prefix("nama-"))
        .and_then(|rest| rest.strip_suffix("Z.tar"))
        .is_some_and(|time| DateTime::strptime(NAME_TIME, time).is_ok())
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
    // Beside `to`, so the finished `.tar` is renamed into place, never copied. Off the runtime: the
    // Server's own Backup folder may be a slow share, and playback shares the runtime.
    let parent = to.parent().unwrap_or_else(|| Path::new("")).to_owned();
    let staging = tokio::task::spawn_blocking(move || TempDir::new_in(parent))
        .await
        .map_err(io::Error::other)??;
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
    // ponytail: an image removed while this runs fails the backup, or leaves the database naming
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
        let mut entry = entry?;
        // `unpack_in` skips a path outside `into` but makes a symlink to anywhere, which the
        // swap would then follow, so a backup holds only files and folders.
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(Error::NotABackup(format!(
                "{} is neither a file nor a folder",
                entry.path()?.display()
            )));
        }
        entry.unpack_in(into)?;
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
        // Copied beside it first, since it may be on another filesystem than the data directory,
        // so a failed copy leaves the config file in place.
        let copy = with_suffix(config_file, ".restoring");
        fs::copy(config, &copy)?;
        move_aside(config_file)?;
        fs::rename(copy, config_file)?;
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
