## Images

The image store: the images the Server keeps, their resized copies and thumbhashes, and the Plugins' Manifest logos kept in it (`plugin_logos.rs`). It keeps the originals through the `Blobs` interface (`blobs.rs`); `LocalDisk` (`local_disk.rs`), files in the data directory, is the one store today. `serving.rs` sends an original from the store with an `ETag` taken from its key, and a resized copy from the local cache, made from the original's bytes (ADR 0052).

- **One owner for stored images.** Only `ImageStore` in `crates/nama-server/src/images.rs` reads or writes the image files in the data directory, tests aside (ADR 0037), and `image_thumbhashes`, tests included. Artwork and later image kinds go through it. The Server makes one `ImageStore`, since each starts its own resize thread and ADR 0039 allows one; its clones share it. Resized copies live in its `resized` folder and uploads arrive in its `uploads` folder, so no key's first part is either. A key names its bytes (a title source's path, or a version taken from them): `ImageStore::write` keeps a file already at its key, and browsers keep what `serve` sends for good. A change that stores or removes Matches has `Artwork` delete the images none stores (`drop_unused`) and download those the new ones store (`download_missing`) once it commits, so of downloaded images the store holds what Matches store and nothing else. Custom Images stay until removed, and a Plugin's Manifest logo until another replaces it, even once its Package is gone.
- **Switching stores.** `switch.rs` copies every original under `ORIGINAL_DIRS` to the new store while every write and delete goes to both, under the key's write lock, and makes the new store current only once the caller has saved it; a failure keeps the old one. A new folder of originals gets its constant in `ORIGINAL_DIRS`, or a switch leaves it behind.
- **Private modules.** The area's files are private modules; neighbours import only what `images.rs` re-exports, and items inside default to `pub(super)`.

### Neighbours

- Uses: `db` and `server_settings` (`ImageSettings`).
- Used by: `artwork` and `custom_images` (the Library area), `plugins` (`clean_svg`, `MAX_SVG_BYTES`), the `api` router and `crates/nama-server/src/api/plugins.rs`, and startup in `lib.rs`.
