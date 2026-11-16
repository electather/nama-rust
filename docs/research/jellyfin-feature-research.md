# Jellyfin vs Nama: what to support, where to catch up, what people ask for

Final report, 2026-11-16. Reviewed by an adversarial pass; Nama facts as of main @ 3a9899a2 (open issues as of 20:00Z). Inputs: ledgers L1 (demand), L2 (clients and member UX), L3 (server and reliability), the Nama inventory, `PRODUCT.md`, `GLOSSARY.md` and the ADRs; findings files were drilled into where a ledger needed checking (G for Fider tables, H for sentiment). Everything here is a suggestion list, not a commitment.

## 1. Summary

- **Nama already beats Jellyfin where Jellyfin is structurally weak.** Requests are native (Jellyfin declined them, Fider #604; the Seerr companion has 13,253 stars). Watch State is keyed by External ID, so a renamed or moved file keeps progress, which Jellyfin's path-hashed ids still lose (jf#12297, #17877 open in 12.0); the cost is that Unmatched Items have no Watch State (X13). The HLS playlist is cut at real keyframes, which designs out Jellyfin's most-thumbed open bug (jf#2547, subtitle desync, 104 thumbs, open since 2020); the cost today is slow first play and resume (#822, #825; G20). Playback Reasons are typed. GPUs are detected and test-encoded, with software fallback (Fider #450, 427 votes; jf#4389). Plugins are WASM against a versioned contract, which avoids Jellyfin's plugin breakage on every release.
- **Nama's biggest credibility gap is reach, not features.** It is web only. Households judge a server by its TV app ("won't switch until" list in H; Fider #612 Apple TV 465, #1615 Vidaa 464, #335 Samsung 255; tizen#222 402 reactions). No TV path at all means a Jellyfin or Plex household rejects Nama outright.
- **List 1 (table stakes) has 15 items, and nine basics are already met.** Most are member-facing browse and player work of size S or M: Next Up, autoplay next, sidecar subtitles and a subtitle-download source, subtitle size and offset, paging, filters, a Watchlist, per-User Library access, a Content Rating cap, Now Playing, backup and UI languages. Large: the TV path, i18n, and the skip-intro detector.
- **The cheapest visible win is "Remove from Continue Watching".** Jellyfin has had it planned since 2020 and declined a 2026 re-ask as a duplicate. In Nama it is one per-User flag (effort S), and it is the #2 vote-getter on Fider (#517, 1,742).
- **Requests are the moat; take them into browsing.** Jellyfin can only show gaps. Nama can fill them: missing Episodes with a Request button, a Watchlist whose missing titles are one tap from a Request, "Part of a collection" with the missing parts requestable, a secondary "Request their other films" action on a person page (never rows of titles you don't have), search misses that fall through to Request search, "Report a problem" (Fider #1254, declined by Jellyfin).
- **Seven Nama traps must be closed before users arrive.** No backup before a migration runs (`db.rs`). No warning when the database sits on a network share. An empty mountpoint removes items (`scanner/walking.rs`). No added time is stored, and #884 must not reuse the highest item id (#871). Future collections, playlists and favourites must be keyed by External ID. Plugin Permissions gate only Automation-lane calls and Built-in Plugins are in-process Rust, so "sandboxed" marketing is wrong (ADR 0006, 0049). Unmatched Items have no Watch State (X13). Most are S effort.
- **Stay Movies and Shows; publish proof, not claims.** Music (Fider #181 gapless 655, open; Feishin 10,427 stars), audiobooks (#243, 859 votes; Audiobookshelf 15,031 stars) and Live TV (theme 904 votes; 72 jf issues) each have a dedicated app. Users run Navidrome, Audiobookshelf and Immich beside Jellyfin (HN 43711706 calls it a "kitchen sink"). Nama's low-memory design answers Jellyfin's OOM and "database is locked" pain (jf#15101, 230 comments; jf#17871, 25 GB heap), but no Nama RSS, large-library or time-to-first-frame numbers exist yet, and Jellyfin 12.0 got much faster (15 s → 1-2 s series loads).
- **Decisions (§7):**

| Decision | Recommended |
|---|---|
| D1 Clients | PWA and Media Session now; AirPlay after X10's signed media URLs; Google Cast after a receiver-hosting spike; then one native Android TV client that shows Requests. A Jellyfin-compatible API shim only as an optional sidecar, never in the Kernel. |
| D2 Remote access | Decide the Relay model now, because it shapes the Hub. First a reachability check, invite links and a reverse-proxy recipe; then Hub-brokered direct connections with per-Server TLS and a Relay fallback, never required (ADR 0002). |
| D3 Scope | Movies and Shows only; a home-video kind only together with share links. |
| D4 Hub | No Hub code before the table stakes; move the Watchlist onto the Server now (a GLOSSARY and ADR change); start the Hub with D2. |
| D5 Plugin UI | Data-only, declared UI Extension Points that Nama renders, with no plugin JavaScript; after home and browse settle. |

## 2. How to read this

**Demand units are never added across kinds:**

| Unit | Meaning | Source |
|---|---|---|
| votes | Fider votes on features.jellyfin.org (`Fider #N`) | G, verified against raw data by L1 |
| thumbs | GitHub thumbs-up on an issue (`jf#N` = jellyfin/jellyfin, `web#`, `androidtv#`, `Swiftfin#`, `tizen#`, `webOS#`, `desktop#`) | E1, E2, F1, F2 |
| reactions | all emoji on an issue (larger than thumbs for the same issue) | C |
| up | GitHub Discussion upvotes (`disc #N`) | B, G |
| stars | GitHub stars of a workaround or companion project | C, D |
| sources | independent community posts or articles (Reddit pts, HN pts) | H |
| comments | issue or post comment count, a measure of heat rather than of demand | various |

**Tiers** follow L1: **S** = 1,000+ votes, or 200+ thumbs/up on one item, or 8+ independent sources. **A** = 400-999 votes, 50-199 thumbs, or 5-7 sources. **B** = 150-399 votes, 15-49 thumbs, or 3-4 sources. **C** = below that. L3 also tiered by stars without stating a scale; it treated about 3,000 combined stars as the S line (it called Intro Skipper's 2,929 "just under"). Where a tier rests on stars or on summed thumbs, the table says so.

**Effort** is my estimate for Nama, not measured: **S** ≤ days, **M** ≤ 2 weeks, **L** ≤ 2 months, **XL** more.

**Caveats.**
- Fider skews to 2019-21: 984 of 1,959 posts were created in 2019-2020. The highest post from 2024 on has 227 votes (Fider #2637). Vote counts overstate how urgent an item is now. I weight 2025-26 sources (H, GitHub discussions) more for current priority.
- Counts overlap. One user votes on many posts. Theme totals in G are keyword-assigned below the top ~330 posts. L2 and L3 theme sums add thumbs across overlapping issues. Duplicate merging is unclear: pre-transcoding is 982 to 1,660 votes depending on whether #284's 678 is already inside #570.
- "Closed completed" and Fider "completed" are not delivery. The 2019 bulk redirect closed many requests, and Fider #330 iOS downloads shows "completed" while the apps still download originals only.
- Stars measure a whole tool, not one feature. Jellystat, for example, does more than statistics.
- Requests demand can't be measured from votes: it routes to Seerr, and Jellyfin declined it.
- Unverified claims are marked *(unverified)*. Claims from general platform knowledge rather than the research files are marked as such.

Where the ledgers disagree on a tier or a status, the table of what I picked is in the Appendix.

## 3. List 1: Must support (table stakes)

Strict test: a Jellyfin or Plex household would reject Nama without it, **and** Jellyfin has it today, at least in part. Everything else is in Lists 2-4.

**Already table stakes in Nama** (met, so not listed below): multiple Users with Permissions; mark played and unplayed (`web/src/components/mark-played-menu.tsx`); Library search (FTS5 with diacritic folding, `migrations/0042_title_search.sql`); Match and Pinned Match (identify); GPU Transcodes; a quality menu (`playback-limit-menu.tsx`) plus a default Playback Limit for remote playback; audio and subtitle Track menus; trusted reverse-proxy headers (ADR 0018); the Sign-in Code.

### Watching on devices

| # | Feature | Why it's table stakes (evidence) | Nama status | Effort |
|---|---|---|---|---|
| T1 | **At least one living-room path**: AirPlay and Google Cast from the web app as the floor (see D1 for more) | H names "Apple TV/Roku/Samsung/LG official, polished apps" as a deal-breaker. Fider #612 Apple TV 465, #1615 Vidaa 464, #335 Samsung 255; tizen#222 402 reactions; Chromecast 153 thumbs over 23 issues (F1); jf#1396 110 comments. Jellyfin ships Android TV, Roku, webOS, Tizen and Swiftfin, plus a Cast sender and receiver. | Absent. Web only; "Cast" absent from the player (`web/src/components/player.tsx`); no issue. | AirPlay M (after X10); Cast M-L |
| T2 | **Installable phone/tablet app**: PWA manifest, standalone display, Media Session (lock-screen and media-key controls) | Jellyfin has Android and iOS apps. Media keys/MPRIS: F1 123 thumbs (desktop#3, 71 thumbs). PRODUCT.md's users are "on the couch" on phones and tablets. | Absent: no manifest or service worker (inventory "Clients"). | S |

### Browse and home

| # | Feature | Why it's table stakes (evidence) | Nama status | Effort |
|---|---|---|---|---|
| T3 | **Library paging, sort, filter (genre, unplayed, year, added) and A-Z jump**, with filters remembered per User | Fider #216 lazy loading 1,186; #276 genre 258; #143 remember filters 73; androidtv sort/list theme 143 thumbs (F2 §3). Jellyfin has 29 sort keys. | Planned: Nama #884, #879, #819, #881, #882; case bug #811. Today `ListMovies`/`ListShows` return the whole Library sorted by title. | M (in flight) |
| T4 | **Next Up in Continue Watching** (the next Episode of a started Show), plus **"Remove from Continue Watching"** | Jellyfin has Next Up. Fider #1055 combine rows 209; #517 remove 1,742; L2 home themes 54 + 95 thumbs. | Partial. Continue Watching is "started and not played" only (`crates/nama-server/src/browse/watching.rs`); no next Episode, no dismiss. Paging #882. | S-M |
| T5 | **Correct Recently Added**: date added per title, not per file; new Episodes grouped under their Show | Fider #4021 154 (2026, a recent high), #2329 "don't show upgraded items" 69; jf "date added" theme 89 thumbs / 328 comments. | Absent. Home has no Recently Added row or hero (#883 closed not planned). Recently Added is planned as a Library sort (#819/#884), which needs `added_at` (#871). | S |
| T6 | **Save titles for later** (Watchlist; Jellyfin uses favourites) | Fider #576 Watchlist 1,307; 8 sources. Jellyfin and Plex both have a "for later" list (favourites / Watchlist). | Absent on the Server. Watchlist is a Hub concept (GLOSSARY "Watchlist" is an Account's list); Hub unbuilt. | M |

### Player and subtitles

| # | Feature | Why it's table stakes (evidence) | Nama status | Effort |
|---|---|---|---|---|
| T7 | **Autoplay the next Episode** with a countdown, plus **"Are you still watching?"** | Jellyfin has both (web; Android TV since 0.19). Fider #814 496 + #3439 111; #400 pre-buffer 223. Binge flow. | Absent (inventory "Player"). | S-M |
| T8 | **Skip intro / credits** | Fider #45 1,089; Intro Skipper 2,929 stars; 7 sources call the plugin "mandatory". Plex Pass has it. Jellyfin has the Media Segments API plus a plugin. | Absent. | M (segments, chapter names, manual) → L (detector) |
| T9 | **Sidecar subtitle files plus a subtitle-download source** (`.srt`/`.ass`/`.vtt` beside the video, with charset detection) | Bazarr (4,429 stars) writes sidecars. Jellyfin reads them and detects charsets. Jellyfin has the official OpenSubtitles plugin and a per-User subtitle-download permission (Fider #185, completed in 10.9); 64 jf issues mention OpenSubtitles. | Absent. Sidecars are only touched when deleting (`scanner/deleting.rs`); no subtitle source. | M (sidecars) + M (download source) |
| T10 | **Subtitle size, background and timing offset**, remembered per User | Fider #161 189, #3036 182; androidtv#2479 32 thumbs; web#558 open since 2019. Jellyfin web has an offset slider and appearance settings. PRODUCT.md: "Captions and subtitle controls are first-class". | Absent. The subtitle menu picks Tracks only. | S-M |

### Users and access

| # | Feature | Why it's table stakes (evidence) | Nama status | Effort |
|---|---|---|---|---|
| T11 | **Per-User Library access** ("friends see Movies, family sees everything") | Jellyfin has EnabledFolders. Fider #493 groups 204; HN 48986091; Wizarr and jfa-go profiles depend on it. | Absent: Permissions (ADR 0020) are Server-wide, and nothing scopes a User to Libraries. Needs an ADR and a GLOSSARY term. | M |
| T12 | **Content Rating cap per User** (a "Kid" preset of Libraries and a max rating) | Jellyfin has max rating and tags. Fider #531 Child Mode 77, #235 86; E1 37 thumbs. Emby gives PINs and rating limits free (XDA 2026-07). | Absent. Content Rating is already stored (`browse.proto content_rating`). | M (together with T11: one read layer) |

### Admin

| # | Feature | Why it's table stakes (evidence) | Nama status | Effort |
|---|---|---|---|---|
| T13 | **Now Playing**: who is watching what, by Direct Play/Remux/Transcode and why, with a stop button | Fider #301 kill stream 402. Jellyfin's dashboard home shows live sessions. | Absent. PlaybackStarted/Ended/Changed events exist; no admin UI. | S-M |
| T14 | **Backup and restore**, plus an automatic copy before migrations | Fider #299 899 (Jellyfin shipped in 10.11), #1603 143, #3546 92; forum 39k views; jf#15027 migration failures 34 thumbs / 121 comments. | Absent. sqlx migrations run on open (`crates/nama-server/src/db.rs`), 75 files, no copy first. | S (`VACUUM INTO` plus the custom-images folder or the blob store, + schedule) to M (restore UI) |
| T15 | **UI languages (i18n)** | Kept on fit, not demand (tier C). The List 1 part is "start string extraction now"; the translations themselves are List 2 (G14). Jellyfin has 105 cultures. Households are non-technical, so an English-only UI excludes non-English families (L2, L3). Fider #370 per-user language 220. | Absent ("English only, with no i18n library", `web/README`). | L (cheaper the earlier it starts) |

**Notes.**
- **T1.** AirPlay: M, after X10's short-lived signed media URLs. Senders: Safari on Apple devices only. Media under `/media/` needs a session (`api.rs`) and the cookie is SameSite=Strict (ADR 0015), while the Apple TV fetches the URL itself without Safari's cookies. *(Cookie behaviour is general knowledge; spike first.)* Cast needs a custom receiver and a reachable Server URL/HTTPS (L2); the receiver's hosting model is unverified (see D1b). Jellyfin's Cast breaks behind reverse proxies (jf#3712, #1396), so use short-lived plan URLs. The fuller TV answer is D1.
- **T4.** Make dismiss clear itself on the next progress report (L1), and handle Specials (cf. Nama #818). Don't make "mark played" the workaround: in Jellyfin it corrupts Watch State. Adding Next Up changes the GLOSSARY definition of Continue Watching ("started and not played").
- **T5.** Store "first available" per title by External ID, mirroring ADR 0013. Requests can also stamp "arrived for you".
- **T6.** Making the Watchlist a Server feature changes GLOSSARY ("an Account's list") and ADR 0020's note. That is decision D4. The surpass move is in List 2 (G4).
- **T8.** Phase 1 is cheap: a segment store per Version in the Kernel, segments from chapter names, an admin "mark intro" action, and the skip button. Phase 2 is an audio-fingerprint detector as a plugin on the Automation lane, run off-peak behind an admin switch (low-memory rule). Jellyfin users report 48 h of analysis (r/jellyfin 1waai3z).
- **T11/T12.** Enforce access in one Kernel read layer that every list, search, image and media route goes through. Jellyfin filters per query and leaks into artwork and search (jf#12855). A "Kid" preset is calmer than Jellyfin's tag lists. Requests from a kid should go to approval.
- **T14.** SQLite `VACUUM INTO` works while serving. Back up the custom-images folder with it, because the README says `custom-images/` exists nowhere else; the blob store (ADR 0052) or the S3 plugin gives off-box copies. Export Watch State by External ID too, which doubles as migration (L1).
- **T15.** Plain-words copy translates well. Start extraction before strings multiply.
- **Not in List 1, and why.** Offline downloads, collections, trickplay, extras and trailers, NFO reading, metadata editing, cast and scores (tier B, now G5), people browse, dual subtitles, SSO and profiles are either things Jellyfin also lacks or does poorly, or not things a household rejects a server over. They are in Lists 2-4.

## 4. List 2: What Jellyfin does better, and how to catch up and surpass

| # | Area | What Jellyfin does | Nama gap | Catch-up step | Surpass move |
|---|---|---|---|---|---|
| G1 | Client reach | 7 official apps (Android, Android TV, iOS/Swiftfin, webOS, Tizen, Roku, desktop) plus a large third-party ecosystem built on its API (Streamyfin 5,366 stars, Findroid 4,309, Wholphin 3,346, Infuse) | Web only. ADR 0007 promises Swift/Kotlin codegen, but `proto/buf.gen.yaml` emits Rust and TypeScript only. | T1, T2; then D1 | Every Nama client shows Requests (four Jellyfin clients bolt Seerr on). Server-side preferences mean a new client inherits Subtitle and Audio Preferences on day one. Sign-in Code with a QR code (Fider #2637, 227). Custom headers and mTLS from day one in any native client (F1 §3, 166 thumbs). |
| G2 | Offline downloads | Android and iOS download the original file; no transcoded or batch download | None | A "download" Playback Plan: a single MP4 by Remux or Transcode at a Playback Limit, sidecar WebVTT, gated by a new Permission, bounded concurrency; rate-limit downloads per User (Fider #4224) | Whole Season at a chosen quality (Fider #655, 221). Auto-download the next Episode. Offline progress merges by External ID with last-writer-wins on `updated_at`. Shares one pipeline with G16 pre-transcoding. |
| G3 | Collections and playlists | TMDB box sets, "Included In" (12.0, after 5 years), static playlists; smart lists via plugin | None | TMDB `belongs_to_collection` from the title source; "Part of X" on the Movie page, not in the grid | Membership keyed by External ID (Jellyfin's membership breaks on rescan or remount: E2 #23, 61 thumbs / 373 comments; jf#12008, #12734). Missing parts of a collection carry a Request button ("complete the collection"). Smart shelf = a saved T3 filter. |
| G4 | Watchlist and favourites | Favourites heart only; no real Watchlist | None (Hub concept) | Server-local Watchlist (T6) | A Watchlist that holds titles the Server doesn't have. Each is one tap from a Request, and "Available" sends a notice through the existing Notifiers (the Followed Show pattern). Jellyfin can't do this because Requests are out of its scope. |
| G5 | People, scores, browse by actor | Person pages, studios, genres tabs; ratings via plugins | No cast or score fields (`browse.proto` Movie has none) | Top-billed cast and a score on the title page (Fider #463 IMDb/RT 196, tier B; forum ratings thread 12.4k views; M: a Plugin contract field + the TMDB plugin), then a person page listing "In your Library" | "Request their other films" as a secondary action on a person page, never rows of titles you don't have. Cast stored compactly per title, so no global Person graph (Jellyfin's Person tables drove its 25 GB query, jf#17871). |
| G6 | Extras and trailers | ~35 extras rules; trailers via plugin (the channel is gone, jf#2963, 26 thumbs) | Extras detected and skipped (`scanner/names.rs`) | Store extras of a title and show them on its page | On-demand trailer button from the title source's video key, also on the Request page ("should I ask for this?"). Never autoplay (PRODUCT.md anti-reference). |
| G7 | Trickplay and chapters | Server-made thumbnail tiles since 10.9 (heavy CPU and disk; "What is Trickplay?" disc #12632, 13 up) | None | Chapter list from probing, drawn as markers on the scrub bar (cheap) | Keyframe-only thumbnails (jf#11336 claims a 110x speedup) made lazily on first scrub or off-peak, stored in the blob store, behind an admin switch. Nama already lists keyframes per Version. |
| G8 | Skip intro | Media Segments API + Intro Skipper plugin | None | T8 phase 1 | Built in, with a segment-provider Extension Point (like Recognisers) so detectors are swappable. Credits segments trigger autoplay-next (T7) at the right moment. |
| G9 | Metadata editing, NFO, local art | Field editor, locks, NFO read and write, images by URL (Fider #270, 252), bulk edit (#144, 189) | Match / Pinned Match / Custom Images only; no NFO; local artwork planned #732; provider image picker #730 | #732, #730; read NFO **ids only** as an ID Hint | Edits as overlays beside the original, never in place (the ADR 0036 pattern), so a refresh can't wipe them. Jellyfin's do get wiped (jf#6709, #18511). Paste-a-URL Custom Image. |
| G10 | Subtitle ecosystem | Sidecar files, charset detection, OpenSubtitles plugin (276 stars, makes duplicates) | No external subtitles at all | T9: sidecars, plus a SubtitleSource Extension Point that members may call while watching, under a Permission (parity with Jellyfin 10.9's per-User subtitle-download permission, Fider #185) | Whisper generation (subgen 1,519 stars) as an admin-gated heavy job. Offset stored per User per Version on the Server (T10). |
| G11 | Users and sign-in breadth | Library access, parental tags, LDAP plugin, web-only SSO plugin (archived 2026-05), Quick Connect | No Library access, rating cap, SSO, 2FA, invites or picker | T11, T12 | An OIDC Sign-in Method plugin. Because the Kernel owns sessions (ADR 0014), it works on every client, including TVs via the Sign-in Code, which Jellyfin can't do (disc #16470). Passkeys instead of TOTP. Invite links with preset Permissions and expiry. A User picker with a PIN as a convenience lock on an already signed-in Device, never a Sign-in Method (Jellyfin removed PIN login in 10.9 for security). |
| G12 | Admin dashboard | Sessions, activity log, 20 scheduled tasks, logs viewer, API keys, devices | Devices only; Diagnostics planned #862, #863 | T13; an Activity list from the Event stream | Now Playing shows Playback Reasons in plain words. API keys are each a Device of a User with a Permission subset (Jellyfin's keys are unscoped admin keys). "Copy diagnostics" on the Playback info panel, with that plan's ffmpeg log. |
| G13 | Plugin ecosystem size | 36 official plugins, any-URL repositories; awesome-jellyfin 9,905 stars | Plugins bundled at build time; no install RPC; no catalogue | Install from file, then one curated catalogue showing requested Plugin Permissions before enable (ADR 0049) | "Plugins keep working across upgrades", proved by a CI job loading the last release's Packages against main. No restarts (ADR 0028). |
| G14 | Theming, branding, languages | 9 themes, custom CSS, 105 cultures | Dark only, English only | T15, then the translations; Server name and logo | Theme by tokens (ADR 0016), never by DOM, so upgrades don't break themes (#7220 "custom theme not working since 10.11.0"). |
| G15 | SyncPlay (watch together) | Exists on web and desktop; buggy with Transcodes; missing on TV clients | None | Group RPC over the Event stream; the web player only, so one player to sync | Invite link for guests without Library access (Fider #971, 271; disc #7299). Cross-Server parties via the Hub later. Plex removed Watch Together in Feb 2025, so the ground is open. |
| G16 | Pre-transcoding | Missing (Fider #570, 982), but Tdarr/ffmpeg-over-ip companions exist | None | An "optimized Version" made by an off-peak job, admin opt-in | Reuses the Version model and G2's pipeline; serves both remote play and downloads. Fits low-memory hosts better than remote transcoders. |
| G17 | Player breadth | Speed, PiP, many keys, audio normalisation and 5 downmix algorithms, dual subtitles on some clients | No speed, PiP, downmix control or dual subtitles; keys limited to Space/arrows/F/Esc (`web/src/lib/playback.ts`) | `playbackRate`, PiP, C/M/J/L keys and a "?" overlay; two WebVTT tracks for dual subtitles | One dialogue-forward "night mode" downmix as a User preference in the Remux/Transcode audio path (F1 §29, 53 thumbs; jf#537, 35). Warn that canvas-drawn PGS/ASS vanish in PiP. |
| G18 | Home customisation | 10 configurable home sections, 9 section types | Fixed home | — | A "Your Requests arriving" row, unique to Nama. A few member-chosen pinned rows (a Library, a genre, a collection), never promoted rows (PRODUCT.md). |
| G19 | Years of hardening | Codec and device edge cases across many clients; huge libraries in the wild | v0.1.0, no release history, no scale numbers | A 100k-item fixture benchmark in CI; per-browser planner fixtures | Publish RSS and time on a 1 GiB box for idle, scan and Transcode. Publish "never 'database is locked'" with a stress test. |
| G20 | Time to first frame | Jellyfin starts Transcodes at once on a synthetic playlist (though Transcode start failures are an A-tier pain, L3) | Whole-file keyframe scan before the first Remux: 247 s for a 31 GB file, and over about 75 GB the plan fails (#822); a resume starts ffmpeg at 0, 15 s before the resume segment (#825); failed or abandoned scans rerun on every Play (#850, #866) | #822, #825, #850, #866 | Publish time to first frame next to RSS (G19). |
| G21 | Requests parity with Seerr | Missing in Jellyfin; Seerr has quotas, 4K Requests, Issues and a blocklist (L3, A) | Requests have none of these yet; ADR 0020 adds their Permissions with the concepts | Requests parity: quotas, 4K Requests, Issues, blocklist (L3) | "Report a problem" on a title page (Fider #1254, declined by Jellyfin), in the same app members already use. |
| G22 | HDR, Dolby Vision, lossless audio | Has tone-mapping, with ~11 tone-map fields; DV regressions on LG webOS (L3 A; L2 S on clients); lossless passthrough buggy (L2 A) | Software tone-map and DV Base Layer only; on VAAPI an HDR Transcode tone-maps on the CPU with no limit (#823, #824) | HDR→SDR tone-mapping on the GPU (#823, #824) | One good default instead of tone-map knobs (L3). DV and TrueHD/Atmos are limited in browsers, which is an argument for D1e. |

### Where Nama already beats Jellyfin (keep and advertise)

| Strength | Jellyfin's problem (evidence) | Nama (where) |
|---|---|---|
| Requests built in | Declined "out of scope ... use Ombi" (Fider #604); Seerr 13,253 stars; users dislike running two sites | RequestService, shared Requests, Followed Shows, Quality, progress, availability from the Library (ADR 0001, 0012, 0025) |
| Progress survives renames and upgrades | Path-hashed ids: renames and moves lose progress (jf#12297, #17877 open in 12.0); replace was fixed in jf#15899 after jf#15001 (61 thumbs). 12.0 pruning wiped data (jf#17900) | Watch State keyed by External ID (ADR 0013); Unmatched Items have none (X13) |
| No subtitle/AV drift from a fake playlist | jf#2547 104 thumbs / 151 comments, open since 2020; theme 228 thumbs | Playlist cut at real keyframes; "HLS time is the file's time" (`playback/running.rs`). Fix Remux audio step-back #820 before claiming it. Also fix first-play and resume latency (#822, #825, #850, #866) before claiming it. Today a large first Remux can take minutes. |
| "Why is this transcoding?" | Fider #272 stats-for-nerds 219; #1731 playback error detail declined; jf#645 39 thumbs | Typed Playback Reasons, Playback info panel, typed Refusals (ADR 0019, 0031, 0034) |
| GPU setup that works | Fider #450 auto-test 427 (planned since 2020); jf#4389 no HW→SW failover; QSV/VAAPI theme 675 comments | GPU detection, test encodes, GPU picker, mid-run software fallback (`ffmpeg/hardware.rs`, `playback/running.rs`). Gap: an HDR Transcode on VAAPI tone-maps on the CPU with no limit (#823, #824). |
| Remembered audio and subtitles per Show | Remembers tracks between Episodes since 10.8 (#194), but still no second language (#666, 96), preferences stored per device (#469, 260), and buggy per-series memory (L2: 144 + 142 thumbs); disc #15230 22 up; Plex Auto Languages died (r/PleX 1s3t4lw, 459 pts) | Subtitle/Audio Preference with per-title memory, forced/SDH, "only over another language" (ADR 0035) |
| PGS/ASS without Burn-in | Burn-in forces full Transcodes (theme 39 thumbs / 376 comments); CJK ASS squares (web#934) | Client-drawn PGS/VobSub/ASS with embedded fonts (ADR 0033) |
| No "database is locked" | jf#15101 230 comments; 10.11 umbrella ~3,900 thumbs+comments over ~130 issues | WAL, one writer task, 8 readers, bounded queue (`db.rs`, ADR 0005) |
| Reverse proxies that just work | Reverse proxy / KnownProxies / BaseURL theme 47 thumbs / 615 comments; 10.11 regressed it (jf#15056) (L3, A) | Forwarded headers only from `trusted_proxies` (ADR 0018) |
| Fast rescans | Library scans slow or hang: theme 59 thumbs / 667 comments; 2.5 → 30 min in 10.11 (L3, A) | Incremental scans by size and mtime, then Matching and Probing in the background (`scanner/walking.rs`) |
| Plugins survive upgrades | `targetAbi` treadmill: Trakt 23 builds over 15 ABIs; 12.0 needs .NET 10 rebuilds | WASM against a separately versioned contract (ADR 0006, 0041, 0032) |
| An Installed Plugin can't call what you didn't grant | In-process .NET DLLs, no permissions; a bad plugin crashes the server | Plugin Permissions granted by the Admin gate Automation-lane calls (ADR 0049); allowed hosts in the Manifest limit Installed Plugins' HTTP (ADR 0006); a panic disables only that Plugin (`plugins/points.rs`). Built-in Plugins are in-process Rust, so market only this (trap X6). |
| Cleanup that won't re-download | Auto-delete declined as "too risky" (Fider #294); Maintainerr 2,419 + Janitorr 766 stars | Cleanup plugin on the Automation lane; Kernel deletes with preview; Fulfillers stop monitoring first (ADR 0038, 0048) |
| Secure by default | jf#5415 114 thumbs (security collection); api_key in stream URLs (jf#10808); CVE-2026-35031 | Deny-by-default access table (`api.rs`), session-only `/media/`, SameSite=Strict (ADR 0015) |
| Matching you can see | Fider #665 unmatched filter 102; #2560 naming parity 100; 11 sources | Needs a Match with reasons, Pinned Match, ID Hints, Recognisers, parser versioning (ADR 0023, 0050) |
| Versions without manual merge | Merge Versions plugin 453 stars; episode versions only in 12.0 (PR #8004, 100 thumbs) | Versions grouped by Match |
| Preferences follow the User | Fider #469 global defaults 260; display settings stored per device | Track Preferences are stored on the Server per User |
| No client/server version skew in the web app | Dashboard JS errors from skew (theme 62 thumbs) | Web app served by the Server, typed client generated (ADR 0004, 0007); `buf breaking` gate (ADR 0032) |
| Calm admin | 910-line transcoding form, ~45 knobs; settings sprawl | Few settings, GPU detection, PRODUCT.md "admin power, member calm" |
| Free, no account, works offline | Plex push: r/jellyfin 1thnh41 1,585 pts; r/PleX 1thtnsm 2,023 pts | ADR 0002: fully usable on a LAN with no account or internet |

### Jellyfin traps Nama must not repeat

| # | Trap | Jellyfin evidence | Nama risk found | Fix (effort) |
|---|---|---|---|---|
| X1 | **Migrations with no backup or dry run** | 10.11 migrations took hours or got stuck; jf#15027 34 thumbs / 121 comments; jf#18066 (12.x); users pinned 10.10.7 | sqlx migrations applied on open, 75 files, no copy before and no dry run (`crates/nama-server/src/db.rs`) | Copy `nama.db` before any pending migration and keep the last copy; add a `--check-migrations` dry run (S) |
| X2 | **Database on a network or CoW filesystem** | "Won't fix (unsupported)"; users learn after corruption (jf#15101 (LXC/ZFS, 230 comments), jf#6906; E2 #34) | WAL needs local shared memory; no storage check (no nfs/zfs/statfs check found) | Check the data directory's filesystem at startup and say in plain words when it is a network share; publish a supported-storage table (S) |
| X3 | **Empty mount wipes the Library** | Missing-items theme 54 thumbs / 449 comments; jf#1714 18 thumbs / 48 comments; jf#12008 | Unreadable folders are kept, but a mounted-but-empty share reads as empty and removes items (`scanner/walking.rs`). Watch State survives, but Request availability could flip. *(Inferred from code, not tested.)* | Treat "a Library folder that had files and is now entirely empty" as suspect: hold removal and tell the Admin (S) |
| X4 | **"Date added" from the file, not the title** | Theme 89 thumbs / 328 comments; Fider #2329 69 | No added time is stored. Today only Library backdrops use the highest item id, and #884 must not reuse it (#871). Items are replaced when a file changes, so an id-ordered list would show an upgrade as new. | Key "first available" by External ID (S, already #871) |
| X5 | **Per-title data keyed to files** | Collections and playlists vanish on rescan or remount (jf#12008, #12734); favourites lost (jf#14981, 31 thumbs) | Watch State and Custom Images are safe (ADR 0013, 0036), but nothing yet stops future favourites, Watchlist, collections, playlists, ratings or segments from being keyed by Item. | Generalise library/AGENTS.md's Custom Image and Watch State rule: every per-title store keys on External ID, never an Item or file (S) |
| X6 | **"Sandboxed plugins" claims** | Jellyfin plugins run in-process with no permissions | ADR 0006 says Plugins are trusted code. Plugin Permissions (ADR 0049) gate only Automation-lane calls, and allowed hosts (ADR 0006) limit only Installed Plugins' HTTP. Built-in Plugins are in-process Rust. | Market only "an Installed Plugin can't call what you didn't grant" (S) |
| X7 | Unpaged lists | `/UserViews` allocating 25 GB (jf#17871); Kodi sync hammering (jf#15362, E2 #51) | Whole-Library lists today | Land keyset paging (#879) before any third-party client exists (M, in flight) |
| X8 | Access filtered per query | Parental limits leak into artwork and search (jf#12855) | Not built yet | One read layer for T11/T12 (design rule) |
| X9 | Heavy work inside the scan | Trickplay and chapter extraction during scans take days; scan 2.5 → 30 min in 10.11 | Scan never probes inline today | Keep trickplay, intro detection and subtitle extraction out of scans, behind admin switches and "quiet hours" (design rule) |
| X10 | Tokens in URLs | api_key in stream URLs (jf#10808, 20 thumbs) | Media needs a session cookie today | For external players, AirPlay and Cast, use short-lived signed URLs scoped to one Version, never the session token |
| X11 | PIN as a sign-in method | Removed in 10.9 for security; users still ask | Not built | PIN only as a lock on an already signed-in Device |
| X12 | DOM-injection UI plugins | File Transformation (581 stars) underpins Jellyfin Enhanced (2,052) and Home Sections (559) and breaks every release | No UI extension yet | Declared, data-only UI Extension Points (D5) |
| X13 | **Progress only for matched files** | Jellyfin resumes any file | Unmatched Items have no Watch State (ADR 0013): no resume and no Continue Watching for files the title source can't match | Make "Needs a Match" loud on the title page, or keep file-keyed progress until a Match exists |

## 5. List 3: Most in-demand requests and features

Ranked by tier, then by the strongest 2025-26 signal, then by votes. "Fit" is against PRODUCT.md: household members, "your media, nothing else", Requests in browsing, calm admin.

| Rank | Request | Tier | Key numbers | Jellyfin | Nama | Effort | Fit |
|---|---|---|---|---|---|---|---|
| 1 | Easy remote access and sharing with non-technical family | S | 12+ sources 2025-26 (HN 49604861 623 pts; r/PleX 1xby6k4 1,051 pts); Fider #72 582 | missing | Relay designed, unbuilt (ADR 0010) | XL | High (D2) |
| 2 | Offline and transcoded downloads | S | Fider #218 1,828 (#1 overall), #1341 821, #57 521, #655 221; F1 447 thumbs over 30 issues; Swiftfin#57 146 thumbs | partial (originals, Android/iOS) | absent | L | High for phones; needs a PWA or native app (G2) |
| 3 | Remove an item from Continue Watching | S | Fider #517 1,742; 2026-08 re-ask #4116 declined as a duplicate | missing (planned since 2020) | absent | S | High (T4) |
| 4 | Watchlist | S | Fider #576 1,307; 8 sources | missing | Hub concept only | M | High, ties to Requests (T6, G4) |
| 5 | OIDC / SSO on all clients | S | Fider #230 1,216 (+#271 324); disc #16470 233 up (2026); jf#530 109 thumbs | plugin, web only, archived 2026-05 | absent; Sign-in Method (ADR 0014) | M | Medium: admin-facing, must stay invisible to members |
| 6 | TV and client coverage | S | Fider #612 465, #1615 464, #335 255; tizen#222 402 reactions; 14+ sources | partial | web only | XL | Decision D1 |
| 7 | Library lazy loading at scale | S | Fider #216 1,186 (infinite scroll); performance: jf#15141 39, #15352 32, #17871 | partial (started) | planned #884, #879, #819 | M | High (T3) |
| 8 | Skip intro / credits | S | Fider #45 1,089; Intro Skipper 2,929 stars; 7 sources | partial (API + plugin) | absent | M → L | High (T8) |
| 9 | Tolerant matching, Plex naming parity | S | 11 sources; Fider #2560 100, #665 102; jf#4290 25 thumbs | buggy | largely shipped | keep | High: advertise "works with your Plex folders" |
| 10 | Upgrade reliability | S | 10 sources; HN 45781298 382 pts; jf#15027 34 thumbs / 121 comments | buggy | partial (gates, no pre-migration copy) | S | High (X1) |
| 11 | Requests built in | S (stars) / B (votes) | Seerr 13,253 stars; Fider #604 24 (declined) | declined | **shipped** | — | Core |
| 12 | Subtitle automation (sidecar files, download) | S (stars) / B (votes) | Bazarr 4,429 stars; subgen 1,519; Fider #185 118, #2143 61; E1 36 thumbs | partial (plugin) | absent | M + M | High (T9, G10) |
| 13 | 2FA / passkeys | S (2019) | Fider #26 1,110; 2026: disc #17446 10 up, Fider #4174 passkeys 7 | missing | absent | M | Medium; passkeys suit a web app |
| 14 | Pre-transcoding / "Optimize" | A | Fider #570 982 (with #284 678: 982 to 1,660) | missing | absent | L | Medium; admin opt-in (G16) |
| 15 | Backup, restore, settings export | A | Fider #299 899 (completed 2025-11), #1603 143, #3546 92; forum 39k views | has (buggy) | absent | S-M | High (T14) |
| 16 | Watch history | A | Fider #633 835 | missing (planned since 2020) | absent (only `updated_at`) | M | Medium |
| 17 | Smart playlists and collections | A | Fider #49 589 (+#683 147), #593 105; theme 3,164 votes | plugin | absent | M after T3 | Medium |
| 18 | Temporary share links, guest access | A | Fider #72 582, #1313 88; jf#808 36 thumbs; forum 10.9k views | missing | absent | M (reach needs D2) | Medium |
| 19 | Trailers and extras | A | Fider #55 585, #509 73; androidtv#1150 35 thumbs | partial | absent (extras skipped) | M | Fits without autoplay (G6) |
| 20 | Collections (box sets) | A | Fider #540 517 (completed 2026-07), #1577 152; theme 61 thumbs / 373 comments | has, buggy | absent | M | High (G3) |
| 21 | Autoplay next Episode, "still watching?" | A | Fider #814 496 + #3439 111; #400 223 | has | absent | S-M | High (T7) |
| 22 | Personal ratings | A | Fider #1134 462 | missing | absent | S | Medium; thumbs, not stars |
| 23 | Folder view | A | Fider #224 440 (134 comments, the most-discussed UI post) | partial | absent | M | Low: admin-only "Files" view at most |
| 24 | Kill a stream / Now Playing | A | Fider #301 402; #1535 message all users 90 | partial | absent | S-M | High (T13) |
| 25 | User picker with PIN ("profiles") | A (L1) / S (L2 sums) | Fider #2348 158; web disc #7059 55 up; androidtv#3509 53 thumbs | missing on web; PIN login removed in 10.9 | absent | L | High (households) |
| 26 | Invites / self sign-up | A (picked) | Wizarr 3,285 stars, jfa-go 1,191; Fider #494 227 | missing | absent | M | High: "invite, and they can ask" |
| 27 | Watch statistics, "wrapped" | A (picked) | Jellystat 2,515 stars, Tracearr 2,751; E1 5 thumbs | plugin | absent | M | Medium |
| 28 | Chromecast | A | F1 153 thumbs / 23 issues; Swiftfin#271 43 thumbs; Fider #466 105 | buggy | absent | M-L | High (T1) |
| 29 | Media keys / lock-screen controls | A | F1 123 thumbs; desktop#3 71 thumbs | partial | absent | S | High (T2) |
| 30 | SyncPlay / watch together | A | Fider #971 271, #389 164, #740 105; forum 43k views; androidtv#538 65 thumbs | buggy | absent | L | Medium (G15) |
| 31 | Cross-server Watch State, multiple Servers | A (picked) | ~3.7k stars (WatchState 1,615, JellyPlex-Watched 1,064, CrossWatch 1,015); Fider #184 381 (declined), #47 179, #716 105 (a friend's server) | declined / missing | Hub designed | XL | Medium (D4) |
| 32 | Dual subtitles | A | desktop#482 47 thumbs; androidtv#4394 36 | missing on most clients | absent | S-M | Medium |
| 33 | Library cleanup by Watch State | S (stars) / B (votes) | Maintainerr 2,419 + Janitorr 766 + media-cleaner 269 stars; Fider #294 44 (declined) | declined | **shipped** | — | Advertise |
| 34 | External SQL database | A | Fider #315 668; jf#42 72 thumbs | started | absent | XL | Low: the real ask is "no locking on ZFS/NFS"; answer with X2 |
| 35 | Subtitle appearance and offset | B (votes) / S (thumb sums) | Fider #161 189, #3036 182; androidtv#2479 32 thumbs; web#558 open since 2019 | partial | absent | S-M | High: accessibility (T10) |
| 36 | Missing Episodes shown | B | Fider #572 347, #41 195 | partial (TVDB only) | absent | M | High, with a Request button |
| 37 | Original title, multilingual metadata, per-User language | B | Fider #32 323, #370 220, #610 215 | missing | partial (one Server language; English UI) | M-L | Medium (T15) |
| 38 | Versions, editions, 4K label | B | Fider #251 276, #174 266, #1738 146, #2537 105 | partial | partial (Versions shipped; no labels; largest is default) | S | Medium: label `{edition-…}`, pick by Device |
| 39 | Genre search/filter, remembered filters | B | Fider #276 258, #39 172, #143 73 | partial | planned #884 | M | High (T3) |
| 40 | Per-User Library access, parental controls | B | Fider #493 204, #235 86, #531 77; E1 37 thumbs | has | absent | M | High (T11, T12) |
| 41 | Quick Connect with QR | B | Fider #2637 227 (top post since 2024) | partial | Sign-in Code shipped, no QR | S | High |
| 42 | Flatten single-Season Shows | B | Fider #3352 182 (2025-06), #8 165, #332 125 | closed not planned (jf#14291) | absent | S | High: a UI rule, no setting |
| 43 | Cast, people, IMDb/RT scores | B | Fider #463 196; forum ratings 12.4k views | partial | absent | M | Medium (G5) |
| 44 | Home sections, "pick something for us" | B | Fider #1986 173; forum "Featured Content Bar" 20k views; r/jellyfin 1v3lbae 980 pts | partial | partial (Continue Watching, Libraries) | M | Medium: one calm action, no filler rows |

## 6. List 4: What users keep asking for (recurring, multi-year, still unmet)

G counts **54 Fider posts with 150+ votes that are still open, planned or started after 4+ years**. Jellyfin's FAQ says a feature "likely will not be done" until a volunteer picks it up. Its Social Contract rules out paid developers and sponsored features, and `jellyfin.org/roadmap` returns 404.

**Asks already ranked in List 3.** Demand, status and effort are in List 3; this table adds only why Jellyfin hasn't done it and Nama's opening, keyed by List 3 rank.

| List 3 # | Ask | Why Jellyfin hasn't done it | Opening for Nama |
|---|---|---|---|
| 1 | Easy remote access and sharing | Policy: Let's Encrypt declined (Fider #70, "reverse proxies are best practice"); no paid infrastructure under the Social Contract; Quick Connect solves sign-in, not reachability | Relay + Hub Server Addresses (D2) |
| 2 | Offline and transcoded downloads | Bandwidth: 7 separate apps each need it; transcode-to-file needs a server job; #218 "started" since 2019 | One server-side "download" Playback Plan any client can use (G2) |
| 3 | Remove from Continue Watching | Bandwidth: "planned" for 6 years; the 2026 re-ask was closed as a duplicate | S effort; ship it early and say so (T4) |
| 4 | Watchlist | Bandwidth; and without Requests it can't hold titles the server lacks | Watchlist + Request (G4) |
| 5 | OIDC / SSO for every client | Architectural: disc #16470 says it is blocked on an identity rework; TV clients can't follow redirects; the plugin was archived 2026-05 | OIDC Sign-in Method plugin + Sign-in Code covers TVs (G11) |
| 7 | Large-library browsing | Architectural: EF Core multi-join queries; 12.0 improved it a lot | Keyset paging + published benchmarks (T3, G19) |
| 8 | Skip intro | Architectural choice: API in core, detection left to a plugin; 6 years to "completed" | Built in, phase 1 cheap (T8) |
| 13 | 2FA / passkeys | Same identity limits; plugin-only (JellyfinSecurity 335 stars) | Passkeys, gated by Recent Authentication |
| 14 | Pre-transcoding | Bandwidth; still "open" | Optimized Versions (G16) |
| 16 | Watch history | Bandwidth; left to a plugin (Playback Reporting fills part of it) | History table from Playback events; feeds statistics |
| 17 | Smart playlists | Left to the Smart Lists plugin | Saved filters as shelves (G3) |
| 18 | Temporary share links / guest access | Security posture of exposed servers | Signed, expiring link for one Version, reach via Relay |
| 19 | Trailers | Plugin-only; the trailer channel was removed | On-demand trailer, also on Request pages (G6) |
| 20 | Collection sort and nesting | Bandwidth; "Included In" took 5 years | External-ID collections (G3) |
| 22 | Personal ratings | Bandwidth ("planned") | Thumbs on Watch State |
| 23 | Folder view | Open; partial for some types | Admin-only Files view for Matching triage |
| 24 | Kill a stream | Open | Now Playing with stop and message (T13) |
| 25 | User picker with PIN | PIN login removed in 10.9 for security; web#7447 closed "not planned" for process reasons | Picker on a trusted Device, PIN as a lock only (X11) |
| 26 | Sign-up, invites, email reset | Open | Invite link; reset through a Notifier the User already linked |
| 28 | Chromecast beyond Android and web | Proprietary SDK (libre builds skip it); per-client work | Cast from the web app with a custom receiver (T1; hosting model to spike, D1b) |
| 31 | Federation / multiple servers in one view | Declined: "an external tool" | The Hub is that tool (D4) |
| 35 | Subtitle appearance and offset | Implemented per client, inconsistently | Offset per User per Version on the Server (T10) |
| 36 | Missing Episodes | Partial, works via TVDB only | Show the gap with a Request button |
| 37 | Original title, per-User language | Open | Store both titles; per-User display |
| 40 | Parental limits with PIN | 12.0 rating rework regressing; leaks (jf#12855) | Kid preset + one read layer (T12) |
| 42 | Flatten single-Season Shows | Declined in effect | A pure UI rule |

**Asks only in this list.** "Re-asks" counts the separate posts, issues and discussions cited in the ledgers. It is a floor, not a census.

| # | Ask | First → latest | Re-asks cited | Why Jellyfin hasn't done it | Opening for Nama |
|---|---|---|---|---|---|
| U1 | Subtitle desync on resume | 2019 (jf#935) → 2025 (jf#15845, #16647) | 6 (jf#935, #2547, #8403, #11567, #15845, #16647) | Architectural: fixed-segment fake playlist; jf#2547 still open | Designed out; fix #820 and the first-play and resume latency (#822, #825, #850, #866), then claim it |
| U2 | Watch state lost on rename/move | 2019 (jf#904) → 2026 (jf#17877) | 5 (jf#904, #15001, #12297, #17877; disc #6924) | Architectural: item id = hash of path; replace was fixed in jf#15899, rename and move were not | External-ID keying (ADR 0013); extend it to every store (X5); mind Unmatched Items (X13) |
| U3 | Plugins breaking on upgrade | 10.8 → 12.0, every release | many (jf#15320, #17903; 12.0 notes) | Architectural: .NET ABI coupling | Versioned WASM contract (keep) |
| U4 | Transcode cache size cap | 2020-08 → 2026-08 | jf#3929 54 thumbs / 61 comments | Open | Nama has cleanup and keep-behind; add a hard size cap (S) |

### Declined but voted (Jellyfin said no)

From G's table of `status=declined` posts, with Nama's stance:

| Request | Votes | Jellyfin's reason (paraphrased) | Nama stance |
|---|---|---|---|
| Federated servers (Fider #184) | 381 | Better as an external tool | **Opening**: Hub, Links, cross-Server Watch State (D4) |
| Header authentication (#471) | 168 | Hard on TV operating systems | **Opening, cheap**: applies to the web app; native clients use the Sign-in Code. A trusted-header Sign-in Method plugin (S) |
| Upload from the app or site (#543) | 116 | A media server, not a media manager | No: conflicts with ADR 0003; only with a future home-video kind |
| Report a bad movie (#1254) | 84 | Out of scope, third-party tools | **Opening**: "Report a problem" on a title → Request manager queue → optional re-grab via the Fulfiller (ADR 0020 already lists Issues) |
| External media channels (#1022) | 60 | No plans | No: "your media, nothing else" |
| More info on "Playback Error" (#1731) | 45 | Now tracked as a bug | **Already shipped**: typed Playback Reasons and Refusals |
| Auto-delete watched content (#294) | 44 | Too likely to delete media by accident | **Already shipped**: Cleanup with preview and stop-monitoring |
| Let's Encrypt (#70) | 40 | Use a reverse proxy | Decided with the Relay's per-Server certificates (D2); document Caddy meanwhile |
| Wake-on-LAN (#588) | 29 | Not a WoL server | No |
| Remove from Continue Watching, re-ask (#4116) | 25 | Duplicate of #517 | **Opening**: T4 |
| Request from Sonarr/Radarr/Lidarr (#604) | 24 | Out of scope, use Ombi | **Core**: shipped |
| Widevine DRM (#750) | 21 | User-hostile | No |
| Jellyseerr cohabitation (#2160) | 20 | Third-party, no plans | **Core**: shipped |
| WebTorrent (#550), games (#760), ZIP/RAR (#269) | 20, 16, 13 | Out of scope | No |
| Hide users on the login screen by LAN (#386) | 14 | LAN detection caused Emby security incidents | No: the same reason applies |
| GitHub, closed not planned: flatten single-Season (jf#14291), "search is bad and slow" (jf#12228, 37 comments), User picker (web#7447, a process closure) | — | Not planned | **Openings**: flatten (S), add typo tolerance to the existing FTS5 search (S-M), picker (L) |

## 7. Strategic decisions

### D1. Device and client strategy

**Evidence.** Client coverage is S (14+ sources). Households judge the server by its TV app (H #3; the "won't switch until" list). Fider #612 Apple TV 465, #1615 Vidaa 464, #335 Samsung 255; tizen#222 402 reactions; Chromecast 153 thumbs. Jellyfin's TV wrappers inherit browser-player limits (webOS#248 Dolby Vision, 59 thumbs / 128 comments). Third-party Jellyfin clients are fragmenting ("Clients Epidemics", r/jellyfin 1w3i3d5, 896 pts).

| Option | What you get | Cost | Risk |
|---|---|---|---|
| a. PWA + Media Session + AirPlay | Phone and tablet "app" with no store; Apple TV via AirPlay, sent from Safari on Apple devices only | PWA S; AirPlay M, after X10's short-lived signed media URLs | Offline video in browsers is limited (quotas, iOS). The Apple TV fetches media without Safari's SameSite=Strict cookie *(general knowledge; spike first)* |
| b. Google Cast sender + a custom receiver | Chromecast and Google TV from a phone; the receiver reuses the web player and asks for a Playback Plan with Cast's DeviceDecoding | M-L | Receiver hosting model unverified (one registered HTTPS URL; mixed content against LAN http per ADR 0010); Chrome-only sender. Spike before committing. Also: Jellyfin's Cast breaks behind proxies; Google's SDK is proprietary. *(Receiver registration and the Chrome-only sender are general knowledge; C says Jellyfin "bundles" its receiver, unverified.)* |
| c. 10-foot web layout + Tizen/webOS wrappers | Samsung and LG store apps | L each, plus store accounts and processes | The store process took Jellyfin 2023-09 → 2026-03 for Samsung; TV browser codec limits |
| d. Jellyfin-compatible API shim | Infuse, Swiftfin, Findroid, Android TV, Kodi, Roku clients "for free" | L-XL, moving target (~33 controllers; Findroid needs 12.0+) | Coverage is partial even for Silo (1,021 stars, README-only claims). **Jellyfin clients would hide Requests**, Nama's core feature, unless Nama also serves a Seerr-compatible subset, which those four clients already speak (L2 #25: Streamyfin, Wholphin, Moonfin, Fladder). L2 also requires the shim's memory to stay bounded. |
| e. Native Nama app (Android/Android TV first) | Full control; Requests on the TV; mpv/ExoPlayer direct-play breadth; downloads | XL plus ongoing store upkeep; Swift/Kotlin codegen not yet set up (`buf.gen.yaml`) | Team size |

**Recommendation.** Do a now (PWA and Media Session at once; AirPlay once X10 lands) and b next, after a receiver-hosting spike (M-L). Together they reach phones, Apple TV and Google TV/Chromecast with no store submissions *(that Google TV devices take Cast is general platform knowledge, not from the research files)*. Then choose between e (an Android TV app that shows Requests; C judges Android TV "the easiest TV path", unmeasured) and d. I lean to **e first**. If d is ever built, make it an optional, separately versioned sidecar or plugin, never part of the Kernel API (ADR 0007), so its churn can't leak into Nama. Skip c unless a contributor owns the store process. Skip DLNA (C demand; Jellyfin demoted it to a plugin). Any TV path widens the product beyond PRODUCT.md's "laptop, tablet or phone", so it needs a PRODUCT.md Users update.

### D2. Remote access and sharing with family (Relay / Hub)

**Evidence.** This is the #1 reason people stay on or return to Plex (S, 12+ sources; HN 49604861 623 pts). TVs, Roku and consoles can't run a VPN, CGNAT rules out a reverse proxy, Cast and AirPlay fail over VPN (HN 46517636), and exposing a server directly is unsafe (CVE-2026-35031; r/jellyfin 1ttatmj: 8 of 12 reachable servers vulnerable). Fider #72 share links 582. ADR 0010 defers browser remote access to "the Relay milestone, which is where per-Server certificates get decided"; ADR 0002 says the Relay carries media, never the Hub.

| Option | Cost | Trade-off |
|---|---|---|
| a. Documented reverse proxy / Tailscale / Cloudflare tunnel only (status quo) | S | H: "a weekend project"; fails CGNAT and TVs. Doesn't answer the #1 blocker. |
| b. Self-hostable Relay (the Admin runs it on a VPS) | M-L | Still technical; keeps privacy and cost with the Admin |
| c. Nama-run Hub + Relay (the Plex model, with `*.plex.direct`-style per-Server certificates) | XL plus ongoing hosting, bandwidth and DNS/certificate operations | Answers the blocker fully; puts cost and trust on the project; must never become required (ADR 0002) |
| d. Hub-brokered direct connection with per-Server TLS, and a Relay only as the fallback | XL plus ongoing DNS/ACME operations (as in c), plus L for the Relay; needs an ADR amending 0010 (it puts "a DNS server and certificate issuing inside the Hub") | Most traffic goes direct; the Relay load is small; the Relay software can be open and self-hostable |

**Recommendation.** Make the decision now, because it fixes the Hub's shape (D4), but build it after the table stakes. Meanwhile ship three things: a "can this Server be reached from outside?" check on the admin page, invite links (List 3 #26), and a one-page Caddy/Tailscale recipe (S-M). Then **d**: Hub Server Addresses (LAN plus remote, which also fixes LAN/remote switching, F1 §21, 67 thumbs) with per-Server certificates, and a Relay fallback whose software anyone can run, and which Nama runs only if hosting cost is covered. "Works when your ISP is down" must stay true.

### D3. Scope expansion (music, audiobooks, live TV, photos)

| Media | Demand | Who already does it well | Recommendation |
|---|---|---|---|
| Music | S (stars): Fider #181 gapless 655 (open); Feishin 10,427 stars, Finamp 4,590; HN 43711706 (Navidrome preferred). (5,244 is music, audiobooks and podcasts combined.) | Navidrome, Plexamp, Subsonic clients | **Out.** If it ever comes, a Subsonic API reuses those clients (the same logic as the shim). |
| Audiobooks | A-S: Fider #243 859; jf#52 60 thumbs; Audiobookshelf 15,031 stars | Audiobookshelf | **Out** |
| Podcasts | A: Fider #48 567 (2018-12 → 2019, then redirected) | Dedicated apps | **Out** |
| Live TV / DVR / IPTV | A: theme 904 votes; Fider #186 378 (108 comments); jf live-TV 72 issues (35 open); 7 sources say it keeps people on Plex or Channels | Channels, Plex | **Out.** It is Jellyfin's least reliable area. If ever, as a Plugin outside the Kernel. |
| Virtual channels | S by stars (Tunarr 2,731, ErsatzTV 3,045) | Those tools | **Out**; a cheap "shuffle a Show" covers part of the lean-back need |
| Photos / home videos | B/C: Fider #596 92, #579 home video 91 (completed); Jellyfin maintainers point to Immich | Immich | Photos **out**. A home-video kind is the one plausible later addition, tied to share links (List 3 #18). |
| Books / comics | B: Fider #219 165, #792 121 | Calibre-Web, Kavita *(not in sources)* | **Out** |

**Recommendation.** Keep Movies and Shows. Users run Navidrome, Audiobookshelf and Immich beside Jellyfin (HN 43711706 calls it a "kitchen sink"). Staying out concedes households that need DVR or music: H's "won't switch until" list also names reliable OTA/DVR and Plexamp-level music. PRODUCT.md's success test ("faster to find and start something") favours focus. Keep `LibraryKind` closed. Note in the README that Nama sits beside those apps rather than replacing them.

### D4. Hub priority

**Evidence.** The Hub promises Accounts, Links, Server discovery, cross-Server Watch State, a Watchlist and (through D2) remote reach. Demand behind it: cross-server sync ~3.7k stars, federation (Fider #184, 381 votes, declined by Jellyfin), multiple servers (#47, 179), Watchlist (#576, 1,307). Cost: XL. `crates/nama-hub/src/main.rs` is a stub, and no open issue tracks it.

| Option | Cost | Trade-off |
|---|---|---|
| a. Build the Hub next | XL | Delays every table-stakes item; the Hub's biggest value (reach) still waits on D2 |
| b. Hub after table stakes; move the Watchlist onto the Server now | M now, XL later | Gets the S-tier Watchlist without the Hub; needs a GLOSSARY and ADR change (Watchlist is "an Account's list") and a later sync design |
| c. Never build the Hub | — | Gives up the Relay story and the cross-Server sync differentiator |

**Recommendation: b.** Keep every per-title store keyed by External ID now (X5), so Hub sync later is a merge rather than a migration. Start the Hub together with D2's direct-connection work, because reach is what members will notice.

### D5. Plugin UI extension

**Evidence.** Jellyfin's most popular UI plugins are DOM hacks on File Transformation (581 stars), which underpins Jellyfin Enhanced (2,052), Home Sections (559), Custom Tabs (207), JS Injector (186) and Pages (195). They break every release (L3, A tier). Jellyfin itself has no UI extension API (Fider #3402 reply: the homepage rework "will allow" plugin content). Nama has none either. ADR 0024 makes Plugins declare Extension Points up front.

| Option | Cost | Trade-off |
|---|---|---|
| a. No UI extension; UI stays first-party | — | Calmest; Plugins can't add visible features beyond settings and notifications |
| b. Declared, data-only UI Extension Points: a home row provider (titles plus a heading), a title-page panel (plain text, links, a Request action), an admin page rendered from a settings-style schema; Nama renders everything | M-L | Keeps PRODUCT.md's look and words under Nama's control; covers Home Sections and Jellyfin Enhanced-style needs; these Extension Points are versioned like other contract services (ADR 0043) |
| c. Plugin-supplied JavaScript or iframes | M | Fast to build; a security and consistency risk; repeats Jellyfin's treadmill |

**Recommendation: b, later.** Wait until home and browse settle (T3-T6, G5), then start with the home row provider, since home sections are the strongest signal (Home Sections 559 stars; Fider #1986, 173). Admin pages already come from schemas.

## 8. Suggested sequencing

Buckets, not dates. Each bucket assumes the previous one is mostly done. Large restructures get a plan and an explicit yes first.

**Now** (mostly S-M; several already in flight), in this order:
1. Traps X1-X3: pre-migration copy, network-filesystem warning, hold removal on an emptied folder.
2. T3 paging, sort and filter (#884, #879, #819, #811, #881, #882), with a stored added time (#871, X4).
3. T4 Next Up + Remove from Continue Watching (List 3 #3).
4. T7 autoplay next + "still watching?".
5. First-play and resume latency (#822, #825, then #850, #866; G20).
6. T9 sidecar subtitles and a subtitle-download source; T10 subtitle size, background and offset.
7. T13 Now Playing with stop; T14 backup and restore.
8. X10 short-lived signed media URLs; then T2 PWA + Media Session and the AirPlay part of T1.

Alongside, with no ranking: the X5 rule in library/AGENTS.md, the X6 wording, a "Needs a Match" notice for X13; small wins (QR for the Sign-in Code, List 3 #41; flatten single-Season Shows, #42; fix #820 then claim U1); decide D1-D5 (no code).

**Next**
- T11 + T12 per-User Library access and Kid preset, through one read layer (X8).
- T6 / G4 Server-local Watchlist with Request (after D4's GLOSSARY change).
- G5 cast and score on title pages; G3 collections ("Part of", missing parts requestable); missing Episodes with Request (List 3 #36).
- T8 phase 1 skip intro (segments, chapters, manual); G7 chapter markers.
- The Cast part of T1 (sender + receiver), after the receiver-hosting spike (D1b).
- T15 i18n scaffolding.
- G11: OIDC Sign-in Method plugin, passkeys, invite links, User picker with PIN lock.
- G21 Requests parity: "Report a problem" (Issues), quotas, 4K Requests, blocklist (L3).
- G22 HDR→SDR tone-mapping on the GPU (#823, #824).
- Watch history table (List 3 #16), feeding statistics later.
- Watch State import from Plex and Jellyfin by External ID (switchers' biggest sunk cost, L1).
- Add typo tolerance to the existing FTS5 search; published benchmarks (G19, G20).

**Later**
- D2/D4: Hub, Server Addresses, per-Server TLS, Relay fallback; then share links with reach (List 3 #18) and SyncPlay with guests (G15).
- G2 downloads (transcoded, Season batch) and G16 optimized Versions.
- D1 step 2: native Android TV app (or a shim sidecar).
- T8 phase 2 detector plugin; G7 lazy trickplay.
- D5 UI Extension Points; G13 install from file + curated catalogue.
- Smart shelves (saved filters), statistics / "wrapped", Trakt scrobbling as an EventObserver plugin.

## 9. Out of current scope

| Item | Tier | Evidence | Note |
|---|---|---|---|
| Music (gapless, Plexamp-class) | S (stars) | Fider #181 gapless 655 (open); Feishin 10,427 stars, Finamp 4,590 (5,244 votes is music, audiobooks and podcasts combined) | D3 |
| Audiobooks | A-S | Fider #243 859; Audiobookshelf 15,031 stars | D3 |
| Podcasts / internet radio | A | Fider #48 567 | D3 |
| Live TV / DVR / EPG / IPTV | A | theme 904 votes; 72 jf issues; jf#13147 146 comments | D3 |
| Virtual channels | S (stars) | Tunarr 2,731, ErsatzTV 3,045 | "Shuffle a Show" instead |
| YouTube / channel archiving | A (stars) | ytdl-sub 3,041, Youtarr 1,820 | Possible later as a Fulfiller + title source plugin pair treating a channel as a Followed Show |
| STRM / cloud-drive libraries | A (stars) | AutoFilm 1,060, docker-xiaoya 3,188 | Low priority |
| Adult / regional metadata | S (stars), segment-specific | MetaTube 4,563; MetaShark 2,179 | Not core; a title source plugin anyone can write (ADR 0043, 0044) |
| Books, comics | B | Fider #219 165, #792 121 | D3 |
| Photos | B/C | Fider #596 92; maintainers point to Immich | D3 |
| Voice assistants | B | Fider #275 243 | Community project over the Event stream and webhook (#154) |
| DLNA | B/C | casting theme; plugin 91 stars | D1: skip |
| External SQL database | A | Fider #315 668 | Answer the real ask with X2 |
| Upload from the app | B | Fider #543 116 (declined) | ADR 0003 |
| Cluster / HA / Kubernetes | C | Fider #873 83 | Single binary + SQLite |
| Real-time upscaling, ABR ladder | C | Fider #729 101, #13 72 | ABR is costly on weak CPUs; maybe from pre-encoded Versions later |
| External streaming services, DRM, WebTorrent, games, archives | C | Fider #1022 60, #750 21, #550 20, #760 16, #269 13 | Jellyfin declined all of them too; "your media, nothing else" |
| Cinema mode / pre-rolls, theme songs, 3D, multi-part stacking | C | Fider #2710 30; themesongs 166 stars | A plugin at most |

## 10. Sources

**Ledgers.** `/home/omid/Code/jellyfin-research/ledgers/L1-demand.md`, `L2-clients-ux.md`, `L3-server-reliability.md`.

**Findings.** `/home/omid/Code/jellyfin-research/findings/` `00-nama-inventory.md`, `A1-jellyfin-server-core.md`, `A2-jellyfin-playback.md`, `B-jellyfin-web.md`, `C-jellyfin-clients.md`, `D-jellyfin-plugins-ecosystem.md`, `E1-jellyfin-server-requests.md`, `E2-jellyfin-pain-points.md`, `F1-web-desktop-mobile-requests.md`, `F2-tv-client-requests.md`, `G-feature-votes-and-forum.md`, `H-community-sentiment.md`. Raw Fider data: `/home/omid/Code/jellyfin-research/data/fider-merged.json` (1,959 posts, 64,338 votes).

**Nama.** `/home/omid/Code/nama-rust/PRODUCT.md`, `GLOSSARY.md`, `docs/adr/0001`-`0053`; code spot-checks in `crates/nama-server/src/db.rs`, `scanner/walking.rs`, `browse/watching.rs`, `crates/nama-hub/src/main.rs`; open issues via `gh issue list` (74 open as of 2026-11-16 20:00Z; #892, #894 and #895 were opened after that and are not counted).

**Key URLs.**
- Fider: https://features.jellyfin.org/posts/218 (offline), /517 (Continue Watching), /576 (Watchlist), /230 (OIDC), /216 (lazy loading), /26 (2FA), /45 (skip intro), /570 (pre-transcoding), /72 (share links), /184 (federation, declined), /604 (Requests, declined), /471 (header auth, declined), /1254 (report a problem, declined), /294 (auto-delete, declined)
- GitHub: https://github.com/orgs/jellyfin/discussions/16470 (SSO, 233 up); https://github.com/jellyfin/jellyfin/issues/2547 (subtitle desync); /issues/15001 (watch state on rename); /issues/15101 (database locked); /issues/15027 (migration failures); /issues/5415 (security); /issues/17871 (25 GB heap); https://github.com/jellyfin/jellyfin-web/discussions/7059 (profile switching); https://github.com/jellyfin/jellyfin-tizen/issues/222 (Samsung store); https://github.com/jellyfin/Swiftfin/issues/57 (iOS downloads)
- Ecosystem: https://github.com/seerr-team/seerr; https://github.com/intro-skipper/intro-skipper; https://github.com/Maintainerr/Maintainerr; https://github.com/wizarrrr/wizarr; https://github.com/morpheus65535/bazarr; https://github.com/IAmParadox27/jellyfin-plugin-file-transformation; https://github.com/arabcoders/watchstate
- Policy: https://jellyfin.org/docs/general/faq/ ; https://github.com/jellyfin/jellyfin-meta/blob/master/policies-and-procedures/jellyfin-social-contract.md ; https://jellyfin.org/roadmap (404)
- Sentiment threads (via H): HN 49604861, 48986091, 45781298, 46517636; r/jellyfin 1thnh41, 1w3i3d5, 1ttatmj, 1xubhnj; r/PleX 1xby6k4, 1thtnsm

## Appendix: Where the ledgers disagree, and what I picked

| Item | L1 | L2 | L3 | Picked | Why |
|---|---|---|---|---|---|
| User picker / PIN ("profiles") | A | S | — | **A**, but treated as high priority | L2's S sums thumbs over many overlapping client issues. The strongest single signal is web disc #7059 at 55 up. H names "Plex PIN-profile household accounts" as a deal-breaker, and PRODUCT.md's users are households, so priority is above the tier. |
| Subtitle offset and appearance | B | S | — | **B** by the votes rule, ranked up on fit | Fider #161 has 189 and #3036 has 182. L2's S comes from summed thumbs (153 + 113). PRODUCT.md makes captions first-class (WCAG 2.2 AA). |
| 2FA | S | C | B | **S by 2019 votes, weak now** | Fider #26 has 1,110 (2019), but 2026 GitHub is 10 up (disc #17446). |
| Requests | B (floor) | B | S | **S** | Seerr has 13,253 stars and is the 2nd most-starred ecosystem project. Votes are low because demand routes there and upstream declined it. |
| Watch statistics | C | — | S | **A** | Jellystat 2,515 and Tracearr 2,751 stars measure whole tools. Fider #633 Watch history (835) supports A. |
| Invites / sign-up | B | — | S | **A** | Wizarr (3,285 stars) does invites only, which is strong. Fider #494 has 227 votes. |
| Cross-server watch sync | B | B | S | **A** | The ~3.7k stars (WatchState, JellyPlex-Watched, CrossWatch) mostly serve migrating between Plex and Jellyfin, not federation. |
| Chromecast | B | A | A | **A** | F1 has 153 thumbs over 23 issues, and the casting theme in L3 has 57 thumbs and 1,010 comments. |
| Audiobooks | A | B | S | **A-S** (out of scope) | Audiobookshelf has 15,031 stars; Fider #243 has 859 votes. |
| Themes / custom CSS | C | C | S | **C for Nama** | The stars (ElegantFin 2,273) measure dissatisfaction with Jellyfin's dated default look, which Nama answers by design. |
| Skip intro status in Jellyfin | G: "completed 2025-02" | has (API) | partial | **partial** | The core ships only the Media Segments API. Detection needs the Intro Skipper plugin (E1, H, L2, L3 agree). |
| "Are you still watching?" in Jellyfin | — | F1 "missing" vs B "has" | — | **has** (web via `stillWatching`; Android TV since 0.19) | B found `src/plugins/stillWatching` in code, and Fider #814 is completed: Android TV 0.19 shipped it (response dated 2025-07-24). |
| Nama Recently Added | shipped (#883) | shipped | at risk | **absent** | #883 was closed not planned on 2026-11-16. Home renders only Continue Watching and Libraries (`web/src/routes/_member/index.tsx`). The inventory's "shipped" was wrong. |
