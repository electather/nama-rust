# Images in Jellyfin and Plex, and the Next.js image optimizer

Research note, 2026-11-11. Primary sources only. What was read:

- **Jellyfin server** at tag `v12.4`, commit `83d0d0e7f76e23e2e085c8ff67d28b772d35967d` (2026-10-26). Jellyfin's version numbers went from 10.11.x to 12.0 (released 2026-09-08).
- **jellyfin-web** at tag `v12.4`, commit `0a580ce8df89282dfd120b6d5efb8e106da23bd5`.
- **Plex**: the official PMS API reference at <https://developer.plex.tv/pms/> (an OpenAPI 3.1 document embedded in the page, `info.version` 1.2.3, read 2026-11-11), support.plex.tv articles, forum posts by accounts titled "Plex Employee", and python-plexapi at commit `aa5bf6742eb806bbbebd2b65dee9fc47a81d92e1`.
- **Next.js** at tag `v16.4.6`, commit `54b876b6b161b0ecda19562eb264bec637e60d6d`.

How to read the citations:

| Prefix | Resolves to |
| --- | --- |
| `JF:` | `https://github.com/jellyfin/jellyfin/blob/v12.4/` + path + `#L` anchor |
| `JFW:` | `https://github.com/jellyfin/jellyfin-web/blob/v12.4/` + path |
| `PMS:` | `https://developer.plex.tv/pms/#tag/<Tag>/operation/<operationId>`; the operation id is given |
| `PPA:` | `https://github.com/pkkid/python-plexapi/blob/aa5bf6742eb806bbbebd2b65dee9fc47a81d92e1/` + path |
| `NX:` | `https://github.com/vercel/next.js/blob/v16.4.6/` + path |

Example: `JF:Jellyfin.Api/Controllers/ImageController.cs#L552-L602` is <https://github.com/jellyfin/jellyfin/blob/v12.4/Jellyfin.Api/Controllers/ImageController.cs#L552-L602>.

Plex is closed source, so each Plex claim carries one of three tags: **documented** (Plex's own docs, spec or staff), **observed** (python-plexapi exercises it, Plex does not document it), **not verified**.

## Summary

1. **Library image.** Jellyfin draws one: a 960x540 PNG of a single random item's backdrop, darkened, with the library's name written across it. It is not a poster grid. Plex shows a built-in type icon and offers a composite grid endpoint; Plex documents no way to give a library a custom picture.
2. **When images are downloaded.** Jellyfin is eager: at metadata refresh it downloads one image per enabled type, TMDB `original` size by default, and stores only that one. Candidates are listed live from the provider when the admin opens the picker. Plex providers hand over URLs; the candidate list is fetched on demand, at most once an hour.
3. **Versioned URLs.** Both version image URLs, neither by content hash. Jellyfin has an optional `tag` query parameter (MD5 of item path plus image mtime) that the server never validates; any non-empty tag turns on one-year `immutable` caching. Plex puts a timestamp as the last path segment.
4. **Auth on image routes.** Jellyfin image GETs need no authentication, and an anonymous caller skips the per-user visibility check an authenticated caller gets. Plex requires `X-Plex-Token` on everything, images included.
5. **Bounding resizes.** Jellyfin takes arbitrary integer sizes. Only since 12.0 is output capped at the source's dimensions; before that an anonymous request could pin a core and 6-12 GB. Plex takes free integers. Next.js rejects any width or quality not on a configured list.
6. **Output format.** Jellyfin negotiates on `Accept`: WebP if offered, else JPEG or PNG. No AVIF output. Plex takes an explicit `format` of jpg, png or ppm; staff call WebP output unsupported and unreliable.
7. **Variant cache.** Jellyfin keys on an MD5 of source path, mtime and every parameter, and evicts by age only (30 days since write, no size cap). Plex evicts weekly, files over a month old, no size cap. Next.js has an LRU disk cache with a byte limit.
8. **Overrides and refresh.** Jellyfin has no image lock. A refresh only fills empty slots, so an upload survives until someone ticks "replace existing images", which deletes it, the custom library image included. A `poster.jpg` beside the media beats a manual choice at the next scan. Plex has per-field locks, and deleting an image locks the field.
9. **SVG.** Jellyfin accepts SVG uploads and rasterises them on the server; that produced a stored-XSS advisory (2024) and an SSRF plus memory-exhaustion advisory (2026). Next.js refuses SVG by default, never rasterises it, and sends a sandboxing CSP with `Content-Disposition: attachment`.
10. **Image by URL.** Jellyfin's admin-only endpoint fetches any URL with no host allowlist and no size cap; the same feature was an unauthenticated SSRF in 2021. Plex's photo transcoder fetches remote URLs by design. Next.js needs an allowlist match, blocks private IPs, caps the body at 50 MB and follows 3 redirects.

## Side by side

| Question | Jellyfin (v12.4) | Plex |
| --- | --- | --- |
| 1. What has images | Every library item, plus users (one), plugins (one), the server (splashscreen). 13 image types; one per type except Backdrop and Chapter. | Items carry thumb, art, clearLogo, squareArt, banner, poster, theme. One selected plus a candidate list. Sections carry thumb, art, composite. |
| 2. Library default | Generated 960x540 PNG: one backdrop, dark overlay, library name. | Type icon `/:/resources/movie.png` plus a composite endpoint. |
| 2. Regeneration | At library refresh, if the generated file is over 7 days old or missing. | Composite is rendered per request; URL carries a timestamp. |
| 2. Admin replaces it | Same endpoints as any item; stored outside the metadata folder, which is how the generator knows to leave it. | No documented way. |
| 2. Admin image removed | File deleted, slot emptied, collage regenerated at next refresh. | Not applicable. |
| 3. Download timing | Eager, at metadata refresh. | Vaguely documented as on add; candidates on demand. |
| 3. Size from TMDB | `original` unless the admin picks a smaller TMDB size. | Not verified (the provider example uses `original` URLs). |
| 3. Candidates stored | No. Only the chosen image. | Not verified. |
| 3. Failed download | 404/403: try next candidate. Other error: stop, slot stays empty, next refresh retries. | Not verified. |
| 4. Durable bytes | `<data>/metadata/library/<2 hex>/<item id>/poster.jpg`, or beside the media if the library option is on. | `Metadata/<Type>/<x>/<hash>.bundle/` (`Contents/_combined`, `Uploads`). |
| 4. Deletable cache | `<cache>/images/resized-images/`. | `Cache/PhotoTranscoder`. |
| 4. DB per image | Path, date modified, type, width, height, blurhash. | Not verified (closed). |
| 5. Override routes | Upload (base64 body), download-by-URL, local file, delete, reorder backdrops. Admin only. | POST raw bytes or `?url=`, PUT to select a candidate, DELETE, local file. Admin, or playlist owner. |
| 5. Protected from refresh by | Nothing specific: refresh fills empty slots only. Whole-item lock exists. | Per-field lock (`thumb.locked` and others). |
| 5. Revert | Delete the image; the next refresh refills the slot. | Unlock, or reselect the original candidate. |
| 6. URL shape | `/Items/{id}/Images/{type}[/{index}]?tag=...&fillWidth=...` | `/library/metadata/{id}/{element}/{timestamp}` |
| 6. Auth on GET | None. | `X-Plex-Token` required. |
| 6. Cache headers | With `tag`: `public, max-age=31536000, immutable` plus ETag. Without: `public` plus Last-Modified. Always `Vary: Accept`. | Not verified. |
| 6. Versioned URLs | Yes, optional, by path+mtime hash. | Yes, by timestamp. |
| 7. Parameters | width, height, maxWidth, maxHeight, fillWidth, fillHeight, quality, format, blur, backgroundColor, foregroundLayer, percentPlayed, unplayedCount. | url, width, height, quality, format, upscale, minSize, rotate, blur, saturation, opacity, background, blendColor, chromaSubsampling. |
| 7. Sizes restricted | No list. Capped at source size since 12.0. | No list. |
| 7. Format choice | `format` param, else `Accept`. WebP, JPEG, PNG. | `format` param. jpg, png, ppm. |
| 7. Library | SkiaSharp 3.119.4, Svg.Skia. | Not verified. |
| 7. Variant cache eviction | Daily task, files written over 30 days ago. No size cap. | Weekly task, files created over a month ago. No size cap. |
| 7. Placeholder | Blurhash, computed at refresh, stored in the DB, sent in item payloads. | Blurhash fields (opt-in) and UltraBlur four-corner colours. |
| 8. Plugin image | File in the plugin folder or embedded resource; served unauthenticated, as-is. | Legacy bundles: `icon-default.png`. Today's providers: no logo field. |
| 9. Worst known problems | Anonymous image access; resize DoS; SVG XSS and SSRF; SSRF by URL; cache poisoning by non-atomic writes. | Remote fetch by design; unreliable WebP output; cache growth. |

---

## 1. What has images

### Jellyfin

- Thirteen image types exist: Primary, Art, Backdrop, Banner, Logo, Thumb, Disc, Box, Screenshot, Menu, Chapter, BoxRear, Profile (`JF:MediaBrowser.Model/Entities/ImageType.cs#L11-L75`).
- An item holds an array of image records. Every type is single except Backdrop and Chapter (`JF:MediaBrowser.Controller/Entities/BaseItem.cs#L2654-L2657`). The refresh code treats Primary, Art, Banner, Box, BoxRear, Disc, Logo, Menu and Thumb as "one per item" (`JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L42-L53`).
- Anything that is a library item can carry images: movies, series, seasons, episodes, box sets, playlists, libraries (`CollectionFolder`), and the by-name entities. The by-name ones have their own routes: `Artists/{name}/Images/...`, `Genres/...`, `MusicGenres/...`, `Persons/...`, `Studios/...` (`JF:Jellyfin.Api/Controllers/ImageController.cs#L786-L1415`).
- A user has exactly one profile image (`JF:Jellyfin.Api/Controllers/ImageController.cs#L96-L149`).
- The server has one image, the login splashscreen: generated after each library scan from up to 30 random movie and series posters rated 13 or under, or replaced by an admin upload (`JF:Emby.Server.Implementations/Library/SplashscreenPostScanTask.cs#L44-L90`, `JF:Jellyfin.Api/Controllers/ImageController.cs#L1647-L1773`).
- A plugin has one image (section 8).

### Plex

- An item's settable elements are `thumb`, `art`, `clearLogo`, `squareArt`, `banner`, `poster`, `theme`; theme is audio. **documented** (`PMS:` Library / `libraryMetadataGetElement`).
- An item payload carries `thumb`, `art`, `banner`, `hero`, `theme`, `composite`, the parent and grandparent variants, and an `Image[]` array of `{type, url, alt}`. **documented** (schema `metadata` in the spec at <https://developer.plex.tv/pms/>).
- The `Image.type` enum in the schema is background, banner, clearLogo, coverPoster, snapshot. The provider section of the same document also uses `backgroundSquare`, which the enum omits. **documented** (<https://developer.plex.tv/pms/#section/API-Info/Metadata-Providers>).
- One image is selected and the rest are candidates: `GET /library/metadata/{id}/posters` (also `arts`, `clearLogos`, `squareArts`, `themes`) returns entries with `ratingKey`, `selected`, `provider`, `thumb`. These plural paths are not in the spec. **observed** (`PPA:plexapi/mixins/resources.py#L163-L165`, `PPA:plexapi/media.py#L1043-L1062`).
- Library sections carry `thumb`, `art` and `composite`. **documented** (`PMS:` Library / `libraryGetSections`, by example).
- Account avatars come from plex.tv, not from the server. **observed** (`PPA:plexapi/myplex.py#L191`).
- Whether the server itself has an image: **not verified**.

## 2. Library images

### Jellyfin

**What it shows by default.** A generated Primary image:

- The provider picks up to 8 random items in the library that have a Primary image. For music libraries it picks artists instead (`JF:Emby.Server.Implementations/Images/CollectionFolderImageProvider.cs#L32-L71`).
- For each it prefers the item's Backdrop, then Primary, then Thumb, local files only (`JF:Emby.Server.Implementations/Images/BaseDynamicImageProvider.cs#L135-L166`).
- It asks for a 960x540 PNG (`JF:Emby.Server.Implementations/Images/CollectionFolderImageProvider.cs#L78-L93`). At that aspect ratio the encoder takes the "thumb collage" path (`JF:src/Jellyfin.Drawing.Skia/SkiaEncoder.cs#L769-L786`).
- That path uses only the first image that decodes. It paints a black canvas, the backdrop scaled to full width, a black overlay at alpha 0x78, then the library name in white at 112 px, shrunk to fit 90% of the width and centred, with right-to-left handling (`JF:src/Jellyfin.Drawing.Skia/StripCollageBuilder.cs#L97-L165`).
- A 2x2 grid exists too, but it is used for playlists, genres, photo albums, artists and user views, not libraries (`JF:Emby.Server.Implementations/Images/BaseDynamicImageProvider.cs#L209-L244`).

**When it is generated.** The provider runs as part of refreshing the library folder itself, which a library scan does.

- It reports a change when the library has no Primary image and at least one child has one, or when the existing image sits inside the item's internal metadata folder and is more than 7 days old (`JF:Emby.Server.Implementations/Images/BaseDynamicImageProvider.cs#L266-L291`, `JF:Emby.Server.Implementations/Images/CollectionFolderImageProvider.cs#L95-L99`).
- It refuses to touch an image that is remote or stored outside the internal metadata folder (`JF:Emby.Server.Implementations/Images/BaseDynamicImageProvider.cs#L79-L99`).
- It writes its output into the internal metadata folder, never beside the library (`JF:Emby.Server.Implementations/Images/BaseDynamicImageProvider.cs#L101-L126`).
- A project contributor states in the issue tracker that library thumbnails are only created on scan, and that they are global rather than per user: <https://github.com/jellyfin/jellyfin/issues/10116>.

**How the admin replaces it.** The dashboard's library card opens the ordinary image editor on the library's item id (`JFW:src/apps/dashboard/features/libraries/components/LibraryCard.tsx#L122-L124`), so it is the same upload, download and delete endpoints as any item (section 5).

- "Generated" and "custom" are told apart by file location alone; there is no flag. A library always counts as "save metadata locally" (`JF:MediaBrowser.Controller/Entities/CollectionFolder.cs#L196-L199`), so an upload lands in the library's own folder under the server's `root` directory rather than in `metadata/` (`JF:MediaBrowser.Providers/Manager/ImageSaver.cs#L93`, `#L537-L563`). That is outside the internal metadata folder, so the generator skips it. This is read from source, not run.
- An image file named `poster`, `folder`, `cover` or `default` at the top of one of the library's media folders is also picked up as the library image (`JF:MediaBrowser.LocalMetadata/Images/CollectionFolderLocalImageProvider.cs#L38-L43`).

**When the admin's image is removed.** Deleting removes the file from disk and empties the slot (`JF:MediaBrowser.Controller/Entities/BaseItem.cs#L2380-L2401`). With the slot empty, the generator reports a change and draws a new image at the next refresh of the library folder.

**Trap.** "Replace all metadata" with "replace existing images" on a library deletes the custom image and generates a new one: the refresh removes every image that is in the metadata folder or belongs to an item that saves locally, and libraries always do (`JF:MediaBrowser.Providers/Manager/MetadataService.cs#L112-L118`, `JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L74-L103`). Open issue: <https://github.com/jellyfin/jellyfin/issues/16528>.

### Plex

- Defaults, by the spec's example: `thumb=/:/resources/movie.png`, `art=/:/resources/movie-fanart.jpg`, `composite=/library/sections/{id}/composite/{n}`. The same pattern exists for series, artist and photo. **documented** (`PMS:` Library / `libraryGetSections`).
- `GET /library/sections/{sectionId}/composite/{updatedAt}` builds a grid on request. Options: rows, cols, width, height, border, type, format (jpg or png), crop (center or top), repeat, media (thumb, art or banner), backgroundColor, plus a media query. The spec marks it admin-only. **documented** (`PMS:` Library / `librarySectionGetComposite`).
- The path parameter is named `updatedAt` and described as cache busting, but in the spec's example the number equals the section's `scannedAt`, not its `updatedAt`. **documented, by example** (same operation and `libraryGetSections`).
- Collections have the same endpoint (`PMS:` Content / `libraryCollectionCollectionGetComposite`). A collection's default poster shows up to four images from its items and can be replaced by editing the collection. **documented** (<https://support.plex.tv/articles/201273953-collections/>).
- A custom library picture: `PUT /library/sections/{sectionId}` takes name, scanner, agent, language, locations and prefs, and nothing for artwork (`PMS:` Library / `librarySectionPutSection`). The spec lists `GET /library/sections/{sectionId}/arts`, summarised "Set section artwork" but described as a read; no write operation is documented (`PMS:` Content / `librarySectionGetArts`). **documented, as an absence**. python-plexapi has no setter either. **observed**.

## 3. Where images come from and when

### Jellyfin

**Timing: eager.** Images are fetched during metadata refresh, not on first view.

- For each remote image provider, the refresh lists the provider's candidates and downloads at most one image per single type, and backdrops up to the library's limit (`JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L275-L346`).
- A provider is skipped entirely when the item already has every enabled image type that provider supports and enough backdrops, unless the refresh was asked to replace images (`#L292-L297`, `#L356-L374`).
- A second path stores only the URL (a "stub") for live TV programmes, items that are not files, and by-name items other than music artists (`#L623-L666`). The code comment on the normal path says prefetched images are always wanted.
- Stubs do not wait for a viewer either: every item save converts remote image paths to local files and computes dimensions and blurhash (`JF:Emby.Server.Implementations/Library/LibraryManager.cs#L2532-L2657`, `#L2795-L2803`). The image route converts on request only as a fallback (`JF:Jellyfin.Api/Controllers/ImageController.cs#L1894-L1897`).

**Size from TMDB.** `original`, unless the admin sets a smaller size in the TMDB plugin settings.

- The five size settings (poster, backdrop, logo, profile, still) have no default value (`JF:MediaBrowser.Providers/Plugins/Tmdb/Configuration/PluginConfiguration.cs#L103-L126`).
- An unset size falls back to `original` (`JF:MediaBrowser.Providers/Plugins/Tmdb/TmdbClientManager.cs#L579-L582`).
- Candidates narrower than the library's minimum width are skipped. The default minimum for a movie backdrop is 1280 (`JF:MediaBrowser.Model/Configuration/TypeOptions.cs#L14-L66`).

**Which types by default.** For a movie: one Backdrop, one Primary, one Thumb, one Logo. Art, Disc and Banner are off (`JF:MediaBrowser.Model/Configuration/TypeOptions.cs#L14-L66`).

**Candidates.** Only the chosen image is stored. The candidate list is fetched live from the providers when asked (`GET /Items/{itemId}/RemoteImages`, `JF:Jellyfin.Api/Controllers/RemoteImageController.cs#L62-L118`). The server once proxied candidate thumbnails; those endpoints were removed after CVE-2021-29490 (section 9).

**Failed download.**

- 404 or 403 on a candidate: move to the next candidate (`JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L580-L585`).
- Any other non-success status, or a transport error: stop trying for that image type (`#L587-L591`, `#L614-L617`).
- An exception in the provider is logged and counted as a failure on the refresh result (`#L340-L345`).
- Nothing is stored for a failed image, so the slot stays empty and the next refresh tries again.
- For stubs: if every URL returns 404 or 403 the image record is removed so it is not retried forever; other HTTP errors leave the URL in place (`JF:Emby.Server.Implementations/Library/LibraryManager.cs#L3751-L3803`, `#L2576-L2601`).

### Plex

- A support article says posters are downloaded from internet sources when an item is added. **documented**, vaguely (<https://support.plex.tv/articles/201272763-edit-details/>).
- A metadata provider returns `thumb` and `art` as publicly accessible URLs plus an `Image[]` array, and should offer a `/library/metadata/{id}/images` endpoint listing every candidate. The spec's example uses `https://image.tmdb.org/t/p/original/...` URLs. **documented** (<https://developer.plex.tv/pms/#section/API-Info/Metadata-Providers>).
- The server calls the provider's images endpoint on demand, when the admin opens the artwork picker, and at most once an hour per item; refreshing the item resets that. **documented, staff post** (<https://forums.plex.tv/t/announcement-custom-metadata-providers/934384/25>, <https://forums.plex.tv/t/announcement-custom-metadata-providers/934384/46>).
- Candidate keys use the schemes `metadata://`, `upload://`, `media://`, or a plain URL when the file is not stored locally. **observed** (`PPA:plexapi/media.py#L1072-L1084`).
- `GET /library/metadata/{ids}/file?url=` serves a bundle file for a `metadata://` or `media://` URL. **documented** (`PMS:` Library / `libraryMetadataGetFile`).
- A video's `thumb` falls back to a frame grabbed from the video after a refresh. **documented** (`PMS:` Library / `libraryMetadataDeleteElement`).
- Local files are picked up on the fly when the server notices the folder's time changed, or by the "Refresh local metadata every three days" task. **documented** (<https://support.plex.tv/articles/201553286-scheduled-tasks/>).
- Whether the server downloads every candidate or only the selected one at refresh, at what size, and what happens when a download fails: **not verified**.

## 4. Where the bytes live

### Jellyfin

**Durable.**

- Internal metadata folder: `<data>/metadata/library/<first two hex of item id>/<item id>/` (`JF:MediaBrowser.Controller/Entities/BaseItem.cs#L944-L961`, `JF:Emby.Server.Implementations/ServerApplicationPaths.cs#L36`).
- File names there: `poster` (`folder` for albums, artists, photo albums, people), `backdrop`, `logo`, `banner`, `landscape` for Thumb, `clearart` for Art, `disc` or `cdart`, `back` for BoxRear. The extension comes from the MIME type (`JF:MediaBrowser.Providers/Manager/ImageSaver.cs#L396-L566`).
- Beside the media, when the library option "save artwork into media folders" is on (`JF:MediaBrowser.Model/Configuration/LibraryOptions.cs#L61`, `JF:MediaBrowser.Providers/Manager/ImageSaver.cs#L93-L119`). Names then follow the Kodi-style convention: `poster.jpg`, `fanart.jpg`, `season01-poster.jpg`, `<episode file>-thumb.jpg` (`#L413-L500`, `#L595-L715`).
- Saving a new image deletes the previous file when it was in the metadata folder or the item saves locally (`#L177-L222`).
- User profile images: `<config>/users/<username>/profile<ext>` (`JF:Jellyfin.Api/Controllers/ImageController.cs#L128-L140`).
- Splashscreen: `<data>/data/splashscreen.png`, or the uploaded `splashscreen-upload<ext>` (`#L1661-L1675`, `#L1737-L1740`).

**Database, per item image** (table `BaseItemImageInfos`): id, item id, path, date modified, image type, width, height, blurhash as bytes (`JF:src/Jellyfin.Database/Jellyfin.Database.Implementations/Entities/BaseItemImageInfo.cs#L10-L58`). A path that starts with `http` marks a remote stub (`JF:MediaBrowser.Controller/Entities/ItemImageInfo.cs`). A user image records only path and last-modified (`JF:src/Jellyfin.Database/Jellyfin.Database.Implementations/Entities/ImageInfo.cs#L10-L53`).

- Width, height and blurhash are recomputed when they are missing or the file's mtime differs from the stored date by more than a second (`JF:Emby.Server.Implementations/Library/LibraryManager.cs#L2532-L2553`).

**Deletable cache.** Resized variants live under `<cache>/images/resized-images/<first hex char>/<md5><ext>` (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L77`, `#L511-L548`, `JF:Emby.Server.Implementations/AppBase/BaseApplicationPaths.cs#L55`). The cache directory defaults to `$XDG_CACHE_HOME/jellyfin` on Linux (<https://jellyfin.org/docs/general/administration/configuration/>).

### Plex

- Artwork lives in "bundle" packages; most disk use is under the `Metadata` and `Media` folders. **documented** (<https://support.plex.tv/articles/201553286-scheduled-tasks/>, <https://support.plex.tv/articles/202529153-why-is-my-plex-media-server-directory-so-large/>).
- A staff example path: `Metadata/Movies/<n>/<n>.bundle/Contents/_combined/posters/tv.plex.agents.movie.<id>`. **documented, staff post** (<https://forums.plex.tv/t/no-more-4k-poster-jacket-in-plex/939937/6>).
- `metadata://x` maps to `Contents/_combined/x` in the item's bundle, `upload://x` to `Uploads/x`, `media://x` to `Media/localhost/x`. **observed** (`PPA:plexapi/media.py#L1072-L1084`).
- `Cache/PhotoTranscoder` holds resized images and is safe to delete by hand. **documented, staff post** (<https://forums.plex.tv/t/huge-size-of-phototranscoder-folder/631581/7>).
- Weekly maintenance tasks: remove old bundles (for items no longer in the library), remove old cache files, optimize database. **documented** (scheduled-tasks article; task names `CleanOldBundles`, `CleanOldCacheFiles` in `PMS:` Butler / `butlerPostTask`; `PUT /library/clean/bundles` in `PMS:` Library / `libraryPutCleanBundles`).
- Item payloads carry image paths such as `/library/metadata/{ratingKey}/thumb/{n}` and `UltraBlurColors` with four hex corner colours. **documented, by example** (the spec).
- `thumbBlurHash` and `artBlurHash` are opt-in fields. **observed** (`PPA:plexapi/base.py#L610-L619`). A `GenerateBlurHashes` maintenance task is named in the spec. **documented**.
- What the database stores per image, the `_stored` folder, and whether `_combined` is symlinks: **not verified**.

## 5. Overriding

### Jellyfin

Every write route below needs the `RequiresElevation` policy (administrator), except the user's own profile image.

| Way | Endpoint | Body and limits |
| --- | --- | --- |
| Pick a provider candidate | `GET /Items/{itemId}/RemoteImages` (any signed-in user) lists them; `POST /Items/{itemId}/RemoteImages/Download?type=&imageUrl=` stores one. | No body. The server fetches the URL. |
| Give a URL | The same download endpoint. It accepts any URL, not only provider URLs. | No host allowlist, no size cap; the whole response is read into memory. The response must have an `image/*` content type, or a file extension that implies one. |
| Upload a file | `POST /Items/{itemId}/Images/{imageType}` or `.../{imageType}/{imageIndex}` | The body is the file base64-encoded. The `Content-Type` header must be `image/*` and decides the stored extension. No size attribute on the route. |
| Drop a file beside the media | None; found at scan. | Extensions png, jpg, jpeg, webp, tbn, gif, svg. |
| Reorder backdrops | `POST /Items/{itemId}/Images/{imageType}/{imageIndex}/Index?newIndex=` | |
| Delete | `DELETE /Items/{itemId}/Images/{imageType}[/{imageIndex}]` | Deletes the file from disk, including a file beside the media. |
| User profile image | `POST /UserImage?userId=`, `DELETE /UserImage` | Base64 body. The user themself or an admin. |
| Splashscreen | `POST /Branding/Splashscreen`, `DELETE /Branding/Splashscreen` | Base64 body. |

Sources: `JF:Jellyfin.Api/Controllers/RemoteImageController.cs#L62-L171`; `JF:MediaBrowser.Providers/Manager/ProviderManager.cs#L190-L257`; `JF:Jellyfin.Api/Controllers/ImageController.cs#L85-L86` (base64), `#L96-L149`, `#L297-L458`, `#L1721-L1773`, `#L2072-L2089`; `JF:MediaBrowser.Controller/Entities/BaseItem.cs#L58-L59`, `#L2380-L2401`.

- **SVG is accepted.** Any `image/*` type passes the upload check (`JF:MediaBrowser.Model/Net/MimeTypes.cs#L200-L201`), `.svg` is a supported local extension, and the 2026 advisory states that SVG is accepted for user images (<https://github.com/jellyfin/jellyfin/security/advisories/GHSA-cf3c-8m59-2vhx>).
- **The web client** restricts the file picker to `image/*` and nothing more (`JFW:src/components/imageUploader/imageUploader.template.html#L25`, `JFW:src/components/imageUploader/imageUploader.js#L89`).
- **Local file names.** Primary: `poster`, `folder`, `cover`, `default`, plus `movie` or `show`. Backdrop: `backdrop`, `fanart`, `background`, `art`, numbered variants, and an `extrafanart` folder. Logo: `logo`, `clearlogo`. Thumb: `landscape`, `thumb`. Also `banner`, `clearart`, `disc`. Each may stand alone or be a suffix after the media file name (`JF:MediaBrowser.LocalMetadata/Images/LocalImageProvider.cs#L24-L61`, `#L185-L254`, `#L314-L344`; documented at <https://jellyfin.org/docs/general/server/media/movies/>).

**Protection from the next refresh.** There is no image lock.

- The lockable fields are Cast, Genres, ProductionLocations, Studios, Tags, Name, Overview, Runtime, OfficialRating. None is an image (`JF:MediaBrowser.Model/Entities/MetadataField.cs`).
- Protection is a side effect: a remote provider only fills a type the item lacks (`JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L320`).
- The whole-item lock stops remote image providers unless the image refresh mode is full (`JF:MediaBrowser.Providers/Manager/ProviderManager.cs#L438-L441`).
- "Replace existing images" removes and refetches everything, uploads included (`JF:MediaBrowser.Providers/Manager/MetadataService.cs#L112-L118`; refresh options at `JF:Jellyfin.Api/Controllers/ItemRefreshController.cs#L65-L84`).
- A local file beats a manual choice: at each scan, if a recognised file exists beside the media and the current image is a different path, the local file takes the slot (`JF:MediaBrowser.Providers/Manager/ItemImageProvider.cs#L440-L478`). Issues: <https://github.com/jellyfin/jellyfin/issues/17780>, <https://github.com/jellyfin/jellyfin/issues/6709> (open since 2021). A fix was proposed and closed unmerged: <https://github.com/jellyfin/jellyfin/pull/17781>.

**Revert to default.** Delete the image. The slot is then empty, so the next refresh downloads the provider's first eligible candidate again. There is no "reset" call.

### Plex

- **Pick a candidate:** `PUT /library/metadata/{ids}/{element}?url=`. **documented** (`PMS:` Library / `libraryMetadataPutElement`). The `url` value is the candidate's `ratingKey`. **observed** (`PPA:plexapi/media.py#L1064-L1070`).
- **Upload or give a URL:** `POST /library/metadata/{ids}/{element}`, with the raw bytes as the body (not multipart, not base64), or `?url=` for a remote image. **documented** (`PMS:` Library / `libraryMetadataPostElement`). python-plexapi posts to the plural paths (`/posters`, `/arts`) instead. **observed** (`PPA:plexapi/mixins/resources.py#L167-L181`).
- **Permission:** generally only the admin; the exception is a playlist the user created. **documented** (same operation).
- **Delete:** `DELETE /library/metadata/{ids}/{element}`, which also locks the field. **documented** (`PMS:` Library / `libraryMetadataDeleteElement`).
- **Locks:** locked fields do not update on metadata refresh. **documented** (<https://support.plex.tv/articles/201272763-edit-details/>). Lock field names are `thumb`, `art`, `clearLogo`, `squareArt`, `theme`, set as `<field>.locked=1`. **observed** (`PPA:plexapi/mixins/resources.py#L148-L157`). A manually picked poster is not supposed to change. **documented, staff post** (<https://forums.plex.tv/t/stop-changing-my-posters/870858/53>).
- **Local files:** accepted formats are jpg, png and tbn. Movie poster names include `poster`, `folder`, `cover`, `default`, `movie`, or the movie's own file name; backgrounds `background`, `fanart`, `art`, `backdrop` or `<name>-fanart`; logos `clearlogo`, `logo`. Extra candidates are numbered `poster-2.png`. Season posters are `SeasonXX.ext`. **documented** (<https://support.plex.tv/articles/200220677-local-media-assets-movies/>, <https://support.plex.tv/articles/200220717-local-media-assets-tv-shows/>).
- The new agents need "Use local assets" in the library's advanced settings; legacy agents need "Local Media Assets" ticked and topmost. **documented** (same articles).
- **Revert:** unlock the field, or reselect the original candidate. **observed** (`PPA:tests/test_mixins.py#L287-L293`). No reset call is documented.
- Whether selecting or uploading locks the image field automatically, upload size limits, and whether SVG is accepted: **not verified**. SVG is not among the documented local formats.

## 6. Serving

### Jellyfin

**URL shapes.** `GET` or `HEAD` `/Items/{itemId}/Images/{imageType}` and `/Items/{itemId}/Images/{imageType}/{imageIndex}`, with query parameters. A legacy all-in-path form also exists. User image: `/UserImage?userId=`. Splashscreen: `/Branding/Splashscreen` (`JF:Jellyfin.Api/Controllers/ImageController.cs#L552-L713`, `#L1475-L1481`, `#L1647-L1650`).

**Authentication: none on image GETs.** Verified three ways.

- The write routes carry `[Authorize]`; the read routes carry none (`JF:Jellyfin.Api/Controllers/ImageController.cs#L552-L557` against `#L352-L353`). The base controller adds none (`JF:Jellyfin.Api/BaseJellyfinApiController.cs#L12-L18`), and the server sets a default policy but no fallback policy (`JF:Jellyfin.Server/Extensions/ApiServiceCollectionExtensions.cs#L63-L68`).
- The August 2026 commit that capped upscaling says in its message that the item image endpoints do not require authentication: <https://github.com/jellyfin/jellyfin/commit/e120b7f2dd986d7f07f6814b6d987bafd46baab8>.
- The project's own tracking issue lists the image controllers as unauthenticated and explains why: the browser loads images itself and sends no credentials (<https://github.com/jellyfin/jellyfin/issues/5415>).

An anonymous caller gets more than a signed-in one. The route looks the item up with the caller's user id; a signed-in user who may not see the item gets 404, but with no user the visibility check passes (`JF:Emby.Server.Implementations/Library/LibraryManager.cs#L1725-L1738`, `#L4132-L4145`). The only barrier is knowing the item id.

**Cache headers** (`JF:Jellyfin.Api/Controllers/ImageController.cs#L2000-L2070`):

| Condition | Headers |
| --- | --- |
| Always | `Vary: Accept`; `Age: <seconds since the file changed>`; `Content-Disposition: attachment` |
| Request has a non-empty `tag` | `Cache-Control: public, max-age=31536000, immutable`; `ETag: "<tag>"`; `Last-Modified`; 304 on a matching `If-None-Match` |
| No `tag` | `Cache-Control: public`; `Last-Modified`; 304 on `If-Modified-Since` |
| Request sent `Cache-Control: no-cache` | `Cache-Control: no-cache, no-store, must-revalidate` |

**Cache busting.** The tag is `MD5(item path + image date-modified ticks)` (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L435-L436`). Item payloads carry it in `ImageTags` and `BackdropImageTags` (`JF:MediaBrowser.Model/Dto/BaseItemDto.cs#L487-L499`), and the web client appends it (`JFW:src/components/cardbuilder/utils/url.ts#L158-L169`).

- The server does not check that the tag matches the image. Any non-empty value earns the immutable headers (`JF:Jellyfin.Api/Controllers/ImageController.cs#L1881-L1886`).
- The tag is not a content hash. Replacing a file with different bytes and the same mtime keeps the tag.

**So: Jellyfin versions its image URLs, optionally, through a query parameter.**

### Plex

- `GET /library/metadata/{ids}/{element}/{timestamp}`; the timestamp is there for client cache management. **documented** (`PMS:` Library / `libraryMetadataGetElement`).
- In the spec's examples the trailing number equals the item's `updatedAt`. **documented, by example**. Whether the server rejects a wrong number: **not verified**.
- Every request needs `X-Plex-Token`, by the spec's document-wide security requirement (shared user or admin). **documented** (<https://developer.plex.tv/pms/#section/API-Info/Headers>).
- Exception: addresses on the "allowed without auth" list connect without authorization. **documented** (<https://support.plex.tv/articles/200430283-network/>).
- Response cache headers: **not verified**; Plex does not document them.

**So: Plex versions its image URLs, always, through a timestamp in the path.**

## 7. Resizing and formats

### Jellyfin

**Parameters** on the item image routes: `maxWidth`, `maxHeight`, `width`, `height`, `fillWidth`, `fillHeight`, `quality`, `tag`, `format`, `percentPlayed`, `unplayedCount`, `blur`, `backgroundColor`, `foregroundLayer`, `imageIndex` (`JF:Jellyfin.Api/Controllers/ImageController.cs#L557-L574`). The user image and splashscreen routes take only `tag` and `format`; their processing options were removed in 10.11 (`#L1481-L1484`, `#L1650-L1652`, <https://github.com/jellyfin/jellyfin/commit/96a05276a627607e246fbc85e19f841ef8d24bf5>).

**Sizes are arbitrary integers.** There is no list.

- Since 12.0 the result is scaled down to fit inside the source's dimensions, so nothing is encoded larger than the original (`JF:MediaBrowser.Controller/Drawing/ImageHelper.cs#L9-L17`, `JF:MediaBrowser.Model/Drawing/DrawingUtils.cs#L113-L133`). The commit is an ancestor of `v12.0` and not of `v10.11.11`.
- The web client asks for whatever the layout needs: card width times device pixel ratio, rounded up, at quality 96 or 90 (`JFW:src/components/cardbuilder/utils/url.ts#L158-L169`, `JFW:src/lib/jellyfin-apiclient/ServerConnections.js#L13-L17`). Different screens therefore produce different variants of the same image.
- Quality defaults to 100 in code when omitted, although the doc comment says 90 (`JF:Jellyfin.Api/Controllers/ImageController.cs#L1910`).
- The original file is returned untouched when quality is 90 or more, the source's format is acceptable, no overlay is asked for and the size already fits (`JF:MediaBrowser.Controller/Drawing/ImageProcessingOptions.cs#L57-L124`). GIFs are always returned untouched (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L140-L144`).

**Output format.**

- An explicit `format` wins. The enum is Bmp, Gif, Jpg, Png, Webp, Svg (`JF:MediaBrowser.Model/Drawing/ImageFormat.cs`).
- Otherwise the `Accept` header decides: WebP if it lists `image/webp`, then JPEG and PNG always, GIF if listed. A query parameter `accept=webp` works too (`JF:Jellyfin.Api/Controllers/ImageController.cs#L1927-L1998`).
- The server picks WebP when both sides support it; PNG when the source has transparency and WebP is out; else the client's first supported format (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L223-L249`).
- The encoder can output WebP, JPEG, PNG and SVG (SVG only from SVG input). There is no AVIF output (`JF:src/Jellyfin.Drawing.Skia/SkiaEncoder.cs#L100-L101`, `#L673-L677`).

**Library.** SkiaSharp 3.119.4, Svg.Skia 3.7.0 for SVG input, BlurHashSharp for placeholders (`JF:Directory.Packages.props#L13-L14`, `#L73-L78`). Concurrent encodes are limited by a semaphore sized from a server setting, or the CPU count when unset (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L68-L74`, `#L201-L205`). If encoding fails, the original is served (`#L215-L220`).

**Variant cache.**

- Key: MD5 of a string made of the source path, quality, source mtime ticks, output format, every size and overlay parameter that was given, and a version constant (currently `4`) (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L33-L34`, `#L281-L380`).
- Lookup is a bare file-exists check, and the encoder writes straight to the final path (`#L195-L213`).
- Eviction: a daily task deletes every file under the cache directory last written more than 30 days ago (`JF:Emby.Server.Implementations/ScheduledTasks/Tasks/DeleteCacheFileTask.cs#L71-L108`). It goes by write time, not last use, so a popular variant is deleted and rebuilt every 30 days. There is no size limit.

**Placeholders.** Blurhash, computed on the server when the image record is refreshed, from a copy scaled to at most 128x128, with component counts chosen from the aspect ratio and capped at 9 (`JF:src/Jellyfin.Drawing/ImageProcessor.cs#L415-L432`, `JF:src/Jellyfin.Drawing.Skia/SkiaEncoder.cs#L293-L308`). It is skipped for SVG. It is stored in the database and sent in `ImageBlurHashes`, keyed by type then tag (`JF:MediaBrowser.Model/Dto/BaseItemDto.cs#L530`).

### Plex

- `GET /photo/:/transcode` takes `url`, `format` (jpg, jpeg, png, ppm; default jpg), `width`, `height`, `quality` (default -1, meaning highest), `background`, `upscale`, `minSize`, `rotate` (obey EXIF), `blur`, `saturation`, `opacity`, `chromaSubsampling`, `blendColor`. Width and height are free integers. **documented** (`PMS:` Transcoder / `imageTranscode`).
- `url` may be a remote URL; a token it needs goes inside that URL. **documented** (same operation).
- Output format comes from the `format` parameter, not from `Accept`. WebP is not officially supported because it proved unreliable, pending a move to a different image library. **documented, staff post** (<https://forums.plex.tv/t/certain-default-artist-images-kill-my-plex-server/932898/6>).
- Which library does the work today: **not verified**.
- Resized files are cached; a weekly task removes those created over a month ago. No size cap is documented. **documented** (<https://support.plex.tv/articles/201553286-scheduled-tasks/>).
- Placeholders: `GET /services/ultrablur/colors?url=` returns four colours taken from an image, and `GET /services/ultrablur/image` renders a PNG gradient from four colours; clients are told to call the latter through the photo transcoder. **documented** (`PMS:` UltraBlur / `ultraBlurGetColors`, `ultraBlurGetImage`).
- A source-size limit on the photo transcoder: **not verified**.

## 8. Plugin images

### Jellyfin

- A plugin's manifest (`meta.json`) has an `imagePath`, which must point inside the plugin's own folder (`JF:MediaBrowser.Common/Plugins/PluginManifest.cs#L102-L108`).
- A plugin repository's catalog entry has an `imageUrl`. At install the server downloads it into the plugin folder under the URL's last path segment, through a temp file and a rename, with no type or size check, and records the path in the manifest (`JF:MediaBrowser.Model/Updates/PackageInfo.cs#L77-L81`, `JF:Emby.Server.Implementations/Plugins/PluginManager.cs#L388-L421`, `#L449-L488`).
- Plugins built into the server embed the image as an assembly resource instead; the TMDB plugin ships an SVG this way (`JF:MediaBrowser.Common/Plugins/PluginManifest.cs#L110-L117`, `JF:MediaBrowser.Providers/Plugins/Tmdb/Plugin.cs#L49`).
- Served by `GET /Plugins/{pluginId}/{version}/Image`, marked `[AllowAnonymous]`, with `Content-Disposition: attachment` and a MIME type taken from the file extension. The file is sent as-is: no resizing, no cache headers set by the action. The version in the path is the only cache key (`JF:Jellyfin.Api/Controllers/PluginsController.cs#L217-L259`).
- Before 12.0 a manifest with a relative `imagePath` could expose any file on the server: <https://github.com/jellyfin/jellyfin/security/advisories/GHSA-c39p-cgf8-866g>.

### Plex

- **Legacy, removed.** Plug-in bundles declared `thumb="icon-default.png"` and `art="art-default.png"` and kept files in `Contents/Resources`. **documented**, in archived Plex developer docs (<https://web.archive.org/web/20140924004131/http://dev.plexapp.com/docs/api/runtimekit.html>). Plex's own bundles ship such files (<https://github.com/plexinc-plugins/ABC.bundle/tree/736b66937f0a2f8a6f8a9379ea27bd62bfe89f34/Contents/Resources>).
- **Today.** A custom metadata provider's root response has identifier, title, version, types and features. There is no logo or icon field. **documented** (<https://developer.plex.tv/pms/#section/API-Info/Metadata-Providers>).
- The legacy URL shape for bundle resources: **not verified**.

## 9. Known problems

### Jellyfin

| Problem | Detail | Source |
| --- | --- | --- |
| Unauthenticated image access | Anyone who can guess or learn an item id can fetch its images and so learn what is in the library. Ids derive from file paths. Listed since 2021; unchanged at v12.4. | <https://github.com/jellyfin/jellyfin/issues/5415> |
| Resize DoS, anonymous | `?width=23100&height=23100` cost 100% of a core for 10-15 minutes and 6-12 GB per request, and changing the size by a pixel missed the cache. Fixed in 12.0 by capping at source size. | <https://github.com/jellyfin/jellyfin/issues/17056>, <https://github.com/jellyfin/jellyfin/commit/e120b7f2dd986d7f07f6814b6d987bafd46baab8> |
| Splashscreen DoS, anonymous | The branding endpoint took arbitrary sizes. Affected 10.10.7 and earlier; fixed in 10.10.8. | <https://github.com/jellyfin/jellyfin/security/advisories/GHSA-rrr6-mvwg-9pg9> |
| SSRF through image-by-URL | `/Images/Remote`, `/Items/RemoteSearch/Image` and `/Items/{id}/RemoteImages/Download` fetched any URL without authentication. CVE-2021-29490, fixed in 10.7.3 by removing two endpoints and requiring admin on the third. The remaining endpoint still has no host allowlist. | <https://github.com/jellyfin/jellyfin/security/advisories/GHSA-rgjw-4fwc-9v96> |
| SVG stored XSS | A user's SVG profile image, opened directly by an admin, could read the access token from local storage. CVE-2024-43801, fixed in 10.9.10 by sending `Content-Disposition: attachment` on every image. | <https://github.com/jellyfin/jellyfin/security/advisories/GHSA-vcmh-9wx9-rfqh>, <https://github.com/jellyfin/jellyfin/pull/12490> |
| SVG SSRF and memory exhaustion | Server-side rasterising resolved external references in SVG: a 16x16 SVG pointing at `/dev/urandom` held 3.8 GB; http references were fetched and rendered into a downloadable image. Reachable by any user through the profile image. Affected versions before 12.0. The fix turns off external resolution and adds a validator. | <https://github.com/jellyfin/jellyfin/security/advisories/GHSA-cf3c-8m59-2vhx>, `JF:src/Jellyfin.Drawing.Skia/SkiaEncoder.cs#L49-L51`, `JF:src/Jellyfin.Drawing.Skia/SvgSecurityValidator.cs` |
| Cache poisoning by non-atomic writes | The cache is trusted on file existence and written in place, so concurrent first requests get a 0-byte image, and an interrupted encode leaves a 0-byte file that is served forever. Open. | <https://github.com/jellyfin/jellyfin/issues/17135> |
| Slow first views | Each uncached resize took about 4.5 s after the SkiaSharp 3 update; a wall of posters took minutes. Fixed September 2026. | <https://github.com/jellyfin/jellyfin/issues/17776>, <https://github.com/jellyfin/jellyfin/pull/17775> |
| Cache growth | No size limit; age-based eviction only (section 7). | `JF:Emby.Server.Implementations/ScheduledTasks/Tasks/DeleteCacheFileTask.cs#L71-L108` |
| Custom images lost | Library image replaced on "replace existing images" (open); manual choice overridden by a local file at scan (open since 2021). | <https://github.com/jellyfin/jellyfin/issues/16528>, <https://github.com/jellyfin/jellyfin/issues/6709> |
| Library image ignores parental controls | The generated image is global, so a restricted user can see a backdrop from a title they may not open. | <https://github.com/jellyfin/jellyfin/issues/10116> |
| Blurhash fails on some logos | Open since 2024. | <https://github.com/jellyfin/jellyfin/issues/12106> |

### Plex

- **Remote fetch is by design.** The photo transcoder's `url` may be any remote URL (section 7). No CVE names it on Plex itself, but Tautulli's CVE-2026-31804 is an unauthenticated SSRF made by forwarding a caller's URL to it (<https://www.cve.org/CVERecord?id=CVE-2026-31804>).
- **Related SSRF and traversal bugs** fixed in PMS 1.43.3.10861: CVE-2026-96655 (authenticated SSRF through the video transcoder path), CVE-2026-96652 (SSRF through `/player/timeline` with any token value), CVE-2026-96651 (path traversal through `/system/agents/media/get`). None is in the image path, but they show the same class of bug (<https://www.cve.org/CVERecord?id=CVE-2026-96655>, <https://www.cve.org/CVERecord?id=CVE-2026-96652>, <https://www.cve.org/CVERecord?id=CVE-2026-96651>).
- **Image-processing crash.** A user thread reports that certain images kill the server; staff reply that the user was requesting WebP, which is not supported because it is unreliable. **documented, staff post** (<https://forums.plex.tv/t/certain-default-artist-images-kill-my-plex-server/932898/6>).
- **Cache growth.** In a 2020 thread about a huge PhotoTranscoder folder, staff acknowledge a cleanup bug with a fix pending and say the folder is safe to empty by hand. **documented, staff post** (<https://forums.plex.tv/t/huge-size-of-phototranscoder-folder/631581/7>).
- **Unwanted poster changes.** Users report selected posters changing; staff say it should not happen and could not reproduce it (<https://forums.plex.tv/t/stop-changing-my-posters/870858/53>).
- SVG handling and unauthenticated image access beyond the allowed-networks setting: **not verified**.

---

## Next.js image optimizer

Read from `NX:packages/next/src/server/image-optimizer.ts`, `NX:packages/next/src/server/image-optimizer/transform.ts`, `NX:packages/next/src/shared/lib/image-config.ts` and the docs source `NX:docs/01-app/03-api-reference/02-components/image.mdx`, published at <https://nextjs.org/docs/app/api-reference/components/image>.

**Request contract.** `GET /_next/image?url=...&w=...&q=...`. All three are required; anything invalid is a 400 with a message (`NX:packages/next/src/server/image-optimizer.ts#L178-L338`).

- `url`: at most 3072 characters; not protocol-relative; not pointing back at `/_next/image`. A relative URL must match `localPatterns` (default: all local images). An absolute URL must be http or https and match `remotePatterns` (default: none allowed).
- `w`: digits only, and must be one of `deviceSizes` or `imageSizes` (`#L286-L299`). Defaults: `[640, 750, 828, 1080, 1200, 1920, 2048, 3840]` and `[32, 48, 64, 96, 128, 256, 384]` (`NX:packages/next/src/shared/lib/image-config.ts#L238-L239`).
- `q`: 1 to 100, and must be in `qualities`, default `[75]` (`#L301-L318`, `image-config.ts#L259`). The docs say the list is required from Next.js 16 so that callers cannot make the server produce more qualities than intended.
- There is no height parameter. Width is the only dimension, aspect ratio is kept, and the image is never enlarged (`NX:packages/next/src/server/image-optimizer/transform.ts#L167-L173`).

**Format negotiation.** The first entry of the `formats` config (default `['image/webp']`; AVIF is opt-in) that the request's `Accept` header contains (`image-optimizer.ts#L103-L106`, `#L320`). With no match the source format is kept, except that WebP and AVIF sources are converted to JPEG for clients that do not accept them (`transform.ts#L257-L266`). Animated images, and SVG, ICO, ICNS, BMP, JXL and HEIC, are passed through unchanged (`transform.ts#L25-L26`, `#L235-L255`). The source type is detected from the file's bytes, not from the upstream header (`transform.ts#L208-L222`).

**Engine.** sharp, with loaders limited to AVIF/HEIF, JPEG, GIF, PNG, SVG, TIFF and WebP, a 7-second timeout, an input limit of 268,402,689 pixels, and concurrency halved (`transform.ts#L88-L130`, `#L157-L164`, `NX:packages/next/src/server/config-shared.ts#L2428-L2431`). If the transform fails, the original bytes are returned (`transform.ts#L303-L313`).

**Response headers** (`image-optimizer.ts#L1121-L1158`):

- `Vary: Accept`
- `Cache-Control: public, max-age=<maxAge>, must-revalidate`. For statically imported images, whose URL holds a content hash: `public, max-age=315360000, immutable`.
- `ETag`, with a 304 on a match.
- `Content-Disposition: attachment; filename="<name>.<ext>"`, from `contentDispositionType`, default `attachment`.
- `Content-Security-Policy: script-src 'none'; frame-src 'none'; sandbox;`, from `contentSecurityPolicy` (`image-config.ts#L255-L256`).
- `X-Nextjs-Cache: MISS`, `HIT` or `STALE`.

`maxAge` is the larger of `minimumCacheTTL` (default 14400 seconds, 4 hours) and the upstream's `s-maxage` or `max-age` (`transform.ts#L204-L207`, `NX:packages/next/src/server/image-optimizer/get-max-age.ts`).

**Disk cache.**

- Location: `<distDir>/cache/images/<key>/<maxAge>.<expireAt>.<etag>.<upstreamEtag>.<ext>` (`image-optimizer.ts#L108-L135`, `#L363`).
- Key: a hash of the cache version (4), the source URL, the width, the quality and the output MIME type (`#L340-L352`). Each format is cached separately.
- Size limit: `maximumDiskCacheSize` in bytes. Over the limit, least-recently-used entries are deleted. Unset, it takes 50% of the disk space free at startup; `0` turns the disk cache off (`#L367-L380`, docs section `maximumDiskCacheSize`).
- The docs state there is no way to invalidate an entry other than changing the `src` or deleting the folder, and recommend a low TTL for that reason (docs section `minimumCacheTTL`).

**Limits on the source.** Body at most `maximumResponseBody`, default 50 MB, else 413. At most `maximumRedirects`, default 3; redirects need not match `remotePatterns`. Hosts that resolve to a private IP are refused unless `dangerouslyAllowLocalIP` is set, and the resolved address is pinned for the connection (`image-optimizer.ts#L788-L829`, `image-config.ts#L250-L253`).

**SVG policy.**

- `dangerouslyAllowSVG` defaults to false; an SVG source then gets a 400 (`transform.ts#L223-L234`).
- When true, the SVG is passed through unmodified. It is never rasterised or sanitised; protection is the two headers above.
- The docs recommend pairing it with `contentDispositionType: 'attachment'` and `contentSecurityPolicy: "default-src 'self'; script-src 'none'; sandbox;"`.
- The `next/image` component skips the optimizer by itself when `src` ends in `.svg`.

---

## Not verified

Jellyfin:

- The published OpenAPI document at api.jellyfin.org was not read; the routes were taken from the controllers it is generated from.
- The effective maximum upload size. The image routes set no limit of their own and the server's Kestrel limits were not traced.
- That an uploaded library image lands under `<data>/root/...` and a media-folder `poster.jpg` becomes the library image: read from source, not run.
- Whether the web client triggers a refresh right after a library image is deleted, or the regenerated image waits for the next scan.
- Whether the upscaling cap or the SVG validator were backported to 10.11.x under different commits.
- The order in which a provider's candidates are ranked, which decides the image chosen at refresh.
- When a Person item is first refreshed, and so when its image is first downloaded.
- Which release first sent `immutable` and `ETag`; only v12.4 was read.

Plex:

- Whether every candidate or only the selected image is downloaded at refresh, at what size, and what happens when a download fails.
- Response cache headers on image routes, and whether the timestamp in the path is validated.
- Upload size and format limits, and whether SVG is accepted or rasterised.
- Whether selecting or uploading an image locks the field automatically.
- Which image library the photo transcoder uses, and any source-size limit.
- What the database stores per image; the `_stored` folder; symlinks in `_combined`.
- The legacy `/:/plugins/{id}/resources/` URL shape.
- Any image for the server itself.
- Several python-plexapi line ranges (`myplex.py`, `base.py`, `tests/test_mixins.py`) were reported by a helper and not re-read; the ranges in `mixins/resources.py` and `media.py` were.
- The archived legacy plug-in documentation and the `plexinc-plugins` bundle link in section 8 were reported by a helper and not re-read.
Next.js:

- How a stale cache entry is served and revalidated; the calling code outside the files listed was not read.
- Values in older Next.js releases. Only v16.4.6 was read.
