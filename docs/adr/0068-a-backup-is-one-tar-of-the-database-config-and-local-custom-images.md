# A backup is one `.tar` of the database, the config and local Custom Images

An Admin backs a Server up with `nama-server backup <path>` and restores it with `nama-server restore <path>` (#924). Scheduled backups will write the same file (#1224). A restore and later versions of Nama depend on what a backup holds and how it's laid out, so that is fixed here.

A backup is one uncompressed `.tar`, since images are already compressed and one file is easy to copy elsewhere. In this order, it holds:

- `manifest.json`: `server_version`, the version of Nama that wrote it, and `highest_migration`, the newest migration its database has had. It comes first, so a restore can refuse the backup before unpacking the rest.
- `nama.db`: a copy of the database made with `VACUUM INTO` on a connection of its own, which is safe while the Server serves, since the database is in WAL mode. The copy is made beside the backup and renamed into place, as the copy before an upgrade is (ADR 0060).
- `nama.toml`: the config file, if the Server has one, whatever it's called.
- `custom-images/`: only while the image store is local disk. When a Plugin's blob store keeps the originals (ADR 0052), they're already off the machine. The copy of the database says which store it is, so the two agree.

Everything else in the data directory stays out: downloaded artwork, resized copies, Transcodes and other caches are fetched or made again when they're missing. A backup holds every Plugin's settings as written, API keys included, so it's as secret as the database.

A restore runs with the Server stopped and refuses while a connection still holds the database's WAL open. It refuses a backup whose `highest_migration` is newer than any this Server knows, before changing anything; an older one is upgraded on the next start, which first copies it to `nama.db.before-upgrade`. The database, the config file and `custom-images/` the backup holds each replace the Server's own, which is kept beside it with `.before-restore` added to its name, replacing what the last restore kept. The database goes into the data directory the current config names, last, so a config file or folder that can't be moved stops the restore before it, and the config file to the one the restore was given; if the restored config names another data directory, the restore warns.

## Considered Options

- **A folder per backup**: rejected. One file is easier to copy off the machine.
- **Compressed (`.tar.gz`)**: rejected. Custom Images are JPEG, PNG or WebP, compressed already.
- **Restoring over the current files without keeping them**: rejected. A wrong backup would lose the Server's data with no way back.
- **Restoring from the web app**: left out. A running Server can't replace its own open database; the CLI restore runs with it stopped.
