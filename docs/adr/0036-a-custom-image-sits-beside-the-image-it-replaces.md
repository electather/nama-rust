# A Custom Image sits beside the image it replaces, and a title's belongs to the title

_Amended by #1750: a Plugin's logo is always its Manifest's, and can no longer be a Custom Image._

An admin can upload a Custom Image for a Movie's, Show's or Season's Poster or Backdrop, or a Library's Backdrop. The Server keeps it beside the image it would otherwise show, never in its place: the downloaded Poster or the newest title's Backdrop is still there underneath, and removing the Custom Image brings it back. A title's Custom Image is kept against the title's external ids, as Watch State is (ADR 0013), not against an Item. Items are replaced whenever a file changes, and a Poster is otherwise copied onto every Item's Match. So a title's Custom Image survives a rename, a rescan, a new Version and the title's last file leaving the Server, and the same Movie in two Libraries shows one.

## Considered Options

- **Replace the image in place**, as Jellyfin does: rejected. Jellyfin has no lock on an image, so "replace existing images" at a refresh deletes an admin's uploads, and removing one leaves nothing to show until the next refresh (`docs/research/jellyfin-and-plex-images.md`).
- **Lock the image's slot**, as Plex does for each field: rejected. A lock is a second piece of state to keep in step with the image, and unlocking still doesn't say what the Server would have shown.
- **Keep it against the Item**: rejected. Replacing a Movie's file with a better copy can take the old Item out in one Scan and bring the new one in the next, which would drop the image.

## Consequences

- An Unmatched Item can't have a Custom Image, as it has no Watch State.
- Correcting a Match shows the new title's images. The old title's Custom Image waits with the old title.
- Only an admin removes a title's Custom Image; its files leaving doesn't. A Library's goes with the Library.
- An upload is taken only for a title some Library has Matched, a Season with a Matched Episode, or a Library the Server has. Keeping an image for something that left doesn't extend to taking a new one for it.
- A Match that downloads its images again changes only the image underneath.
- Everyone sees the same Custom Image. A User can't have their own.
