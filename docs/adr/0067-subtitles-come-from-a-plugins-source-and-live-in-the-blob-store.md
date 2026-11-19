# Subtitles come from a Plugin's source and live in the blob store

A User watching a Version that lacks the subtitles they want can fetch them from an outside service, such as OpenSubtitles.com, which Jellyfin users ask for often (#921). The subtitle source is a new Extension Point, `nama.plugin.v1.SubtitleSourceService` 1.0.0, behind the table-driven adapter (ADR 0043), with a Built-in `SubtitleSource` trait in `nama-plugin-api` mirroring it. Its Plugin searches its outside service for a Version's subtitles and downloads one result. The service's API key and account are the Plugin's settings, which the Admin sets for the whole Server; the Plugin keeps to the service's rate limits, and the Kernel neither limits nor counts downloads.

The Kernel stores each download in the blob store (ADR 0052), under `subtitles/` and keyed to the Version, and the Version lists it as a subtitle Track that plays as a sidecar file does (#920). This amends ADR 0052: a downloaded subtitle counts as one of the Kernel's stored files, not as media. It goes nowhere near the video, so a Library mounted read-only, as the README recommends, still gets one, and the Kernel still writes into a Library's folders only to delete media (ADR 0038). A download is at most `BLOB_MAX_BYTES`, 20 MiB, far above any subtitle, and the Kernel refuses a larger one. A Version's downloads are deleted with it.

A User searches and downloads by hand while watching, and only with one new User Permission, **Download subtitles** (ADR 0020), which covers both. It isn't in the default set a Server starts with, and no User gets it on upgrade. An Admin grants it to a User or adds it to the default set; `ADMIN` passes it as it passes every check.

Left out for now:

- **Automatic downloads**, such as fetching a subtitle when an Item arrives with no Track that matches someone's Subtitle Preference (ADR 0035). Manual is the smaller first step. Automatic downloads might go through the Automation lane (ADR 0048) or a Scan.
- **Per-User quotas.** An outside service's own limits are per API key or account, and the Plugin's settings already hold those.
- **Writing sidecars beside the video**, possibly a later Admin opt-in for a writable Library, so that other apps see the files.

## Considered Options

- **A sidecar beside the video when the Library's folder is writable, else the blob store**: rejected. On the README's read-only mount the fallback would be the usual path, and reading sidecars (#920) would need two sources and a writable-folder branch.
- **Always a sidecar**: rejected. It refuses every download on a read-only mount.
- **A Permission plus a per-User daily quota**: rejected for now. See per-User quotas above.
- **Each User's own account at the outside service, as Personal settings (ADR 0046)**: rejected for now. One account the Admin supplies is the smaller first step.

## Consequences

- Switching blob stores copies `subtitles` along with the other folders ADR 0052 names.
- Other apps reading the Library, such as Bazarr or Jellyfin, don't see downloaded subtitles.
- While the blob store's Plugin is disabled, downloaded subtitles can't be played, just as images can't be read.
