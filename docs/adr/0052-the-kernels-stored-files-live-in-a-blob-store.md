# The Kernel's stored files live in a blob store, on local disk or in a Plugin

ADR 0037 kept every stored image as a plain file in the data directory, behind the image store's own interface, and left storage out of the Extension Points until a second implementation showed what the contract needs. S3 and S3-compatible stores are that second one. A blob store is now the Extension Point for where the Kernel's own stored files live, `nama.plugin.v1.BlobStoreService` 1.0.0, behind the table-driven adapter (ADR 0043), with the Built-in `BlobStore` trait in `nama-plugin-api` mirroring it. It is the last Provider-lane point in the design.

The point has five required rpcs, each one object per call: `Put` (replacing any object at the key), `Get` (a missing key answers `found` false, not a failure), `Delete` (a missing key succeeds), `List` (keys by prefix, at most 1000 a page, an empty token on the last) and `Exists`. A key is `/`-separated, at most 1024 bytes, of ASCII letters, digits and `-_.%/`. An object is at most `BLOB_MAX_BYTES`, 20 MiB, the image store's upload cap: the Kernel never puts a larger one and a Plugin may refuse one, so a call holds at most one such object in memory on a small device. A blob call may run for 60 seconds of the Plugin's own time, since it carries a whole object to or from an outside store.

It holds the Kernel's own stored files: the image store's originals, Custom Images and Plugin logos included. It never holds media files; a Library's folders stay on the Server's own disks.

The design that uses it:

- **`images` defines the interface it needs**, `Blobs`, with the five calls and `put_file` for an upload already on disk, so an upload isn't read back into memory before a local store moves it into place. The local-disk store is `images`' own Built-in default and keeps today's layout under the data directory, so a Server that never picks another store behaves as before. The Plugin-backed store, a `Blobs` over `dyn BlobStore`, lives in the `plugins` area, and the Kernel's startup hands `images` one or the other. `images` stays in the bottom row and never calls `plugins`, so `ROWS` and ADR 0051 don't change.
- **Resized copies stay a local cache** (ADR 0039), made from bytes fetched from the store and rebuilt on demand.
- **An original is served with an ETag taken from its key.** A key names its bytes, so the ETag is the same in every store, and the caching headers stay as they are. The local-disk store hands over its file, so an original there is streamed and answers Range requests, its file's own ETag replaced by the key's. A Plugin store's original is served whole from memory, at most 20 MiB, without Range; at most four originals being sent or resized are held whole at once, and one past that waits. Ponytail ceiling: a streaming get in the contract is the fix, once someone running a Plugin store reports memory pressure.
- **The Admin picks the store as a Server setting.** Switching first probes the new store, writing an object, reading it back and deleting it, and refuses with the reason if any step fails. It then copies every original under the named folders, `artwork`, `artwork-cache`, `custom-images` and `plugin-logos`, one object at a time with progress the Admin can see, while writes and deletes go to both stores. Offered artwork is copied too, since it is served from the store. The switch takes effect only once the copy is done.
- **The old store's objects are left in place.** Nothing deletes them; an Admin who switches back finds them there, and the copy brings them up to date.
- **Disabling the store's Plugin leaves images unreadable** until it is enabled again. Ponytail ceiling: refuse that disable if it bites.

## Considered Options

- **Storage behind `object_store` or OpenDAL inside the Server**: rejected. Neither runs inside an Extism Plugin (`docs/research/rust-crates-for-stored-images.md`), so S3 would be built into the Kernel rather than grow through a Plugin, and each further store would need a Server release.
- **`images` calling the `plugins` area for its store**: rejected. `images` sits in the bottom row (ADR 0051); defining the interface it needs there, and having startup hand it an implementation, keeps the rows as they are.
- **Media files on object storage**: left out on purpose. Playback, Probing and Scans read files by path and seek in them; a store answering whole objects of at most 20 MiB can't serve that.
- **Streaming large objects through several calls**: rejected for now. The image store's originals are capped at 20 MiB, so one call per object keeps the contract small.

## Consequences

- The contract adds `BlobStoreService` 1.0.0 and `CONTRACT_VERSION` goes to 0.4.0, one bump shared with the Library-reference setting kind landing beside it.
- ADR 0037's storage interface becomes this Extension Point; its reasons for serving every image through the Server still hold.
- A store Plugin that is slow or down makes images slow or missing, never media.
- An original in a Plugin store is answered whole, never by Range, and costs up to 20 MiB of memory while it is sent, at most four at once. One in the local-disk store streams and answers Range.
