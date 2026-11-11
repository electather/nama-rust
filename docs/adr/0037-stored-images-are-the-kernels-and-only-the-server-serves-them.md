# Stored images are the Kernel's: files behind its own interface, served only by the Server

The Server keeps every image it shows: the Posters and Backdrops a Match downloads, Custom Images, Plugin logos, and later the resized copies. Storing them is the Kernel's job (ADR 0006 lists storage there), behind one small interface of its own over plain files in the data directory. Every image reaches a client through the Server, under `/media/` and so only with a session (ADR 0007), at a URL that carries a version, so a browser can keep it for a year. Storage is not an Extension Point yet, though an S3 Plugin was the idea that raised the question. With local disk as the only implementation we would be guessing the contract, and a Plugin's contract is public.

## Considered Options

- **A storage Extension Point now**, with local disk as a Built-in Plugin: rejected for now. One implementation doesn't show what the contract needs. Installed Plugins can't use the storage crates either: neither `object_store` nor OpenDAL runs inside an Extism Plugin, so the seam would be the Server's own interface whichever way S3 arrives (`docs/research/rust-crates-for-stored-images.md`).
- **`object_store` or OpenDAL under the interface**: rejected for local disk. Plain files can be handed to `tower-http`, which already answers conditional and range requests, and they need no new crate. `object_store` is the choice if S3 is ever built into the Server itself.
- **Send clients to where the bytes live**, such as a signed S3 URL: rejected. The session check, the headers an SVG needs and resizing would each need a second home.
- **Images without a session**, as Jellyfin serves them: rejected. An anonymous caller there skips the check on what a User may see (`docs/research/jellyfin-and-plex-images.md`).

## Consequences

- What is kept falls into three kinds. Custom Images exist nowhere else and belong in a backup. Downloaded images are fetched again by a Scan. Resized copies can be deleted at any time.
- A downloaded image no Match refers to is deleted. A Custom Image is removed only by an admin (ADR 0036).
- Images of titles the Server doesn't have, shown while finding something to Request, stay in a cache of their own that can be thrown away.
- A later storage Plugin implements the Kernel's interface, and the Server still serves the bytes.
- Any signed-in User who guesses a Plugin's id can fetch its logo, and so learn it is installed.
