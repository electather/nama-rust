## Images

The image store: the images the Server keeps as files in the data directory, their resized copies and thumbhashes, and the Plugins' Manifest logos kept in it (`plugin_logos.rs`).

- **One owner for stored images.** Only `ImageStore` in `crates/nama-server/src/images.rs` reads or writes the image files in the data directory, tests aside (ADR 0037), and `image_thumbhashes`, tests included. Artwork and later image kinds go through it. The Server makes one `ImageStore`, since each starts its own resize thread and ADR 0039 allows one; its clones share it. Resized copies live in its `resized` folder, so no key's first part is `resized`. A change that stores or removes Matches has `Artwork` delete the images none stores (`drop_unused`) and download those the new ones store (`download_missing`) once it commits, so of downloaded images the store holds what Matches store and nothing else. Custom Images stay until removed, and a Plugin's Manifest logo until another replaces it, even once its Package is gone.
- **Private modules.** The area's files are private modules; neighbours import only what `images.rs` re-exports, and items inside default to `pub(super)`.

### Neighbours

- Uses: `db` and `server_settings` (`ImageSettings`).
- Used by: `artwork` and `custom_images` (the Library area), `plugins` (`clean_svg`, `MAX_SVG_BYTES`), the `api` router and `crates/nama-server/src/api/plugins.rs`, and startup in `lib.rs`.
