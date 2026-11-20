# Silo vs Nama: how we compare to the competition

Report, 2026-11-20. Seven analysts read shallow clones of `Silo-Server/silo-server`, `silo-plugin-sdk`, `silo-plugins`, `silo-apple`, `silo-android`, `silo-push-relay`, three plugin repos and `siloserver.org`, side by side with Nama on `web/library-home` @ 742cc1b4. Opus covered architecture, playback and plugins; Sonnet covered libraries, users and auth, clients, and engineering. Headline numbers were spot-checked by hand. Silo's lines of code measure size, not quality, and no Silo build or test was run. Treat these as suggestions, not commitments.

## 1. Summary

- **Silo is the Jellyfin replacement built for scale.** It runs Go on Postgres, Redis, S3 and optional Meilisearch. Its pitch in `AGENTS.md`: "rather than SQLite and local disk… treat a node dying mid-stream as a normal event". Nama makes the opposite bet: one Rust binary on one SQLite file, with Requests at the centre.
- **Silo is 2–10× bigger in every measured respect.**

  | Measure | Silo | Nama |
  |---|---|---|
  | Server code | 840k lines of Go, plus 430k lines of tests | 455k lines of Rust |
  | Web app | ~350k hand-written lines | ~42k lines |
  | SQL migrations | 720 | 52 |
  | `/api/v2` operations | 970 | not compared |
  | Plugin extension points | 15 | 11 |
  | Clients | 5 native platforms, plus Jellyfin and Audiobookshelf compatibility layers | web only |
  | Stars | 1,059 | 0 |
  | Contributors | ~30 | solo |
  | Age at first commit | 6 months (2026-05-23) | 15 days (2026-11-05) |

- **Silo's lead is reach.** It has native iOS, tvOS and macOS apps (App Store), Android and Android TV (Play beta) and Roku. It also serves a Jellyfin 12.1-compatible API, so Infuse, Findroid and VidHub connect on day one. Nama is web-only, and our own Jellyfin research already names reach as the biggest credibility gap.
- **Silo's second lead is playback hardening.** It has a ranked-candidate planner with deterministic replan, capability evidence tiers, distributed stateless transcode nodes, Dolby Vision conversion, surround-to-E-AC-3, trickplay, Chromaprint intro detection, offline downloads and watch-together.
- **Nama wins on design discipline and a few deliberate product choices.**
  - **Requests are the core product.** Silo also has built-in Requests, so this is closer than expected.
  - **Watch State is keyed by External ID.**
  - **Plugins are sandboxed WASM.** They have a host allowlist, granted permissions, memory caps and per-point versions. Silo's plugins are unsandboxed native processes running as root.
  - **Deleting media is coordinated with the *arrs.** Silo bans it permanently.
  - **Ops are simple:** one process, one file, a tar backup.
  - **Decisions are well documented:** 76 ADRs that record rejected options, plus enforced structure tests.
- **The Hub is our only answer to Silo's reach, and it is a 12-line stub.** Silo has no account or identity service: "You don't need a Silo account". The Hub design (ADRs 0011 and 0057: federated identity, per-Server TLS, merged Watch State) has no Silo equivalent, but it exists only on paper.
- **Top recommendations (§9):**
  - Close the cheap correctness gaps Silo already solved: a missing-file grace window, `If-Range` resume, surround-to-E-AC-3, *arr autoscan, a non-goals ADR.
  - Then pick a reach strategy. The Jellyfin-compat sidecar, a native Android TV app, or both: decision D1 in `jellyfin-feature-research.md`.
  - Add parental controls and profiles, and OIDC as an Installed Plugin.
  - Build a signed plugin catalog.

## 2. What Silo is and why it exists

**What it says it is.** "A self-hosted media server for films, series, audiobooks, ebooks, podcasts, and manga." Only Movies and Series are in the 1.0 promise; books are beta (`docs/architecture/v1-scope.md`).

**Who it's for.** Households on a Linux box or NAS. The site pitches:
- "Your data stays with you… You don't need a Silo account", aimed at Plex.
- No subscription.
- "Import your watch history from Plex, Jellyfin, or Emby."
- "Many Jellyfin apps work, including Infuse."

**Why it exists.** Jellyfin's SQLite and local-disk architecture is the target. The author treats scale-out, restart-resilient streaming and node failure as normal events. Jellyfin compatibility is "a long-term commitment as an on-ramp".

**Priorities** (`AGENTS.md`): "Performance and reliability first", then KISS and YAGNI. Before 1.0 the focus is QA, correctness and UX polish.

**Permanent non-goals** (`docs/non-goals.md`):
- **Live TV, IPTV, EPG, DVR and `.strm`.** These are banned in core, plugins and clients alike, because of app-store risk: "a rejection or a takedown lands on the entire client suite."
- **Deleting, moving or renaming media.** Silo is "a media server, not a media manager".

The native apps are the centre of the product, and the non-goals follow from protecting them.

**Business shape.**
- **Licensing.** AGPL-3.0-or-later, plus `PLATFORM-EXCEPTION.md` so the code can link Apple and Google SDKs.
- **Trademark.** Silo Media L.L.C. owns the brand (`TRADEMARK.md`); forks must rename.
- **Funding.** GitHub Sponsors pays for "AI development tooling (Claude, Codex)" and the push relay.

**How it's built.** The project is AI-agent-driven and mostly written by one person: Quick104 made about 2,964 of roughly 4,300 contributions. It moves at 20+ commits a day. PRs are closed by default unless the author already has a merged PR. A feature PR needs an `accepted` issue and before/after evidence, and must disclose the AI harness and model that wrote it. GitHub Actions auto-closes PRs that don't comply.

## 3. Architecture, decision by decision

| Decision | Silo | Nama | Note |
|---|---|---|---|
| Datastore | Postgres (with pgvector), Redis event bus, optional Meilisearch, S3; per-user SQLite being retired by 1.0 | SQLite WAL on the Server, Postgres only on the Hub (ADR 0005) | Silo has to ship `postgres_tune_command.go`, and a `PG_MAJOR` bump forces a dump and restore. Nama has none of this. |
| Topology | One binary with `SILO_MODE=integrated` or `api`, `proxy`, `transcode` | One binary (ADR 0001) | Silo's transcode nodes need only the core's URL and an HMAC key: no database, no secrets. |
| Central service | None | Optional Hub (ADR 0002), currently a stub | Nama's possible edge, not yet built |
| API | REST/OpenAPI `/api/v2` with 970 operations, add-only at 1.0, approvals file; Jellyfin compatibility on :8096; Audiobookshelf compatibility on :13378 | Protobuf + ConnectRPC with generated clients (ADR 0007); `buf breaking` with a declared-break log (ADR 0032) | Comparable discipline; Silo's is more mature |
| Plugins | Native child processes via go-plugin/gRPC, unsandboxed | Extism WASM in-process, with permissions and an allowlist (ADRs 0006, 0049) | §6 |
| Requests | Built in, routed to Sonarr/Radarr by a plugin | Built in, the core of the product (ADRs 0001, 0003) | Rough parity |
| Delete media | Banned | `DELETE_MEDIA` Permission, *arrs told to unmonitor first (ADR 0038) | Deliberate difference |
| Live TV | Banned forever | Out of scope, possible later as a Plugin | Silo's reasoning is worth writing down |
| Media scope | Movies, Series, plus beta audiobooks, ebooks, podcasts, manga | Movies and Shows | Our research says stay narrow |
| Licence | AGPL, platform exception, trademark held by an LLC | AGPL core, MIT/Apache contracts (ADR 0009) | Nama's makes third-party clients and plugins easier |

## 4. Playback, streaming and transcoding

Silo's `internal/playback` alone is about 59k lines. Nama's playback and ffmpeg code totals about 9.4k.

### Silo's pipeline

**Decision.** The client posts a `client_playback_context`. `PlanPlaybackV3` then:
- lists every combination of route, recipe, audio, delivery and subtitle mode;
- sorts them: original first, then copy remux, then encode; progressive before HLS;
- picks the first viable candidate whose attempt key hasn't already failed.

Replan walks down that list, so recovery is deterministic, and `plan_id` is a hash of the plan.

**Capability evidence tiers.**
- `exact` (Android's MediaCodecList) is the only tier that earns audio passthrough.
- `platform_attested` (Apple's AetherEngine).
- `declared` (the web's `isTypeSupported`).

Device quirks are tracked separately.

**Deliveries.**
- `original_http`: byte ranges with a strong ETag and `If-Range`.
- Progressive remux.
- Remux to HLS: fMP4, or MPEG-TS.
- Transcode to HLS.

A copy remux serves a VOD playlist built ahead of time from the keyframe index, because Safari stalls on a growing playlist.

**Restart resilience.** The stream token carries the frozen recipe, so any front end can rebuild ffmpeg after a restart. Revocation uses a Redis deny list.

**Transcode nodes.** Requests to a node are signed with HMAC. Routing reserves capacity and accounts for scratch-disk pressure, a job cap and a GPU gate. A node drains on SIGTERM, and a failed one is detected through `TCP_USER_TIMEOUT`.

**HDR and Dolby Vision.**
- Tone-mapping through `tonemapx`, OpenCL, VAAPI or CUDA, with a per-source preflight.
- DV7 → 8.1 conversion.
- A DV RPU probe. It exists because one bad file produced 376k stderr lines.

**Audio.** Surround is converted to E-AC-3 5.1, because Android TV mixers downmix everything else to stereo.

**Extras.**
- Trickplay sprites.
- Chromaprint plus silence, chapter and tail intro detection, alongside a TheIntroDB plugin.
- Chapter thumbnails and theme songs.
- Offline downloads prepared on nodes.
- Watch-together rooms with Postgres-locked seek barriers.
- A bandwidth probe, BBR on listeners, and per-request stream telemetry.

### Nama's pipeline

**Planner.** It narrows step by step (Direct Play → Remux → Transcode) with typed reasons (ADRs 0027, 0031, 0034).

**HLS.** fMP4, cut at real keyframes from MKV Cues or the MP4 `stss` table (ADR 0066). The segment folder is bounded (ADR 0062).

**Hardware.** VAAPI, QSV, NVENC and VideoToolbox are each test-encoded, with a GPU picker. A transcode that fails partway on hardware **falls back to software**. Video output is H.264 only, and audio is copied or converted to AAC. Dolby Vision profile 5 is refused. Subtitle UX rules are good: a subtitle switch never restarts playback, and appearance and offset are kept per User (ADRs 0033, 0070).

**Not built yet.** Segments (intro and credits) and Preview frames exist only as ADRs (0074–0076).

### Who wins

**Silo wins** on:
- recovery on real devices;
- HDR, Dolby Vision and audio coverage;
- HEVC output;
- trickplay and intro detection;
- downloads and watch-together;
- scale-out.

**Nama wins** on:
- legibility: one planner shape, translatable reasons;
- a hard bound on disk use;
- the mid-run hardware-to-software fallback, which has no Silo equivalent;
- subtitle UX rules.

**Why Silo built it this way.** Its docs repeatedly target Jellyfin parity, multi-replica deployment, and third-party clients that "lie" about what they support. That explains a protocol the server owns end to end, and stateless nodes.

## 5. Libraries, scanning and metadata

| Feature | Silo | Nama | Better |
|---|---|---|---|
| Kinds | Movie, TV, Mixed, Audiobook, Ebook, Podcast, Manga | Movies, Shows | Silo (breadth) |
| Watching | inotify only; per-library switch; scans the narrowest target, widens to a full scan on overflow or a burst over 1,000 | `notify`, cross-platform; 30 s quiet period; full scan every 6 h | Silo on control, Nama on platforms |
| *arr autoscan | Token webhooks, rate-limited, never drops a real delivery | None | Silo |
| Missing files | 24 h grace period: hidden, relinked if the file returns, added date kept | Removed at once | **Silo** |
| Re-parse when the parser or metadata improves | Not found | `PARSER_VERSION` and `METADATA_VERSION` (ADRs 0023, 0040) | **Nama** |
| NFO | A built-in provider at the front of the chain (admin locks beat NFO, NFO beats remote) | Never reads `.nfo` | Silo |
| Ignore files | `.nomedia`, `.ignore`, gitignore patterns | Extras folders only | Silo |
| IDs | Deterministic `movie-tmdb-228064`; `local-<hash>` for unmatched files | Open namespace + value External IDs (ADR 0042) | Nama on extensibility |
| Editions | `{edition-…}` parsing | Versions only | Silo |
| Odd naming schemes | `media_analyzer` plugin point | Recogniser plugins (ADR 0050) | Nama |
| Custom images | — | Kept beside the original, survive file loss (ADR 0036) | Nama |
| Box sets and collections | TMDB box sets; about 100 collection templates with scheduled sync | None | Silo |
| Overlays | Per-profile badges, drawn by the client | None | Silo |
| Ratings | IMDb and TMDB always shown; other sources need an admin to enable them, because the score owners restrict display | MDBList plugin | Silo |
| Search | Postgres full-text search or Meilisearch, plus semantic search | Built-in search over matched titles | Silo |

## 6. Plugin system

| | Silo | Nama |
|---|---|---|
| Runtime | One native process per plugin (hashicorp go-plugin over gRPC). Same OS user, **root** in the shipped image; environment allowlisted; GOMAXPROCS capped at 4; stopped after 30 min idle | Extism WASM inside the server. New instance per call, 5 s or 60 s timeouts, 128 MiB memory cap, HTTP only to allowed hosts, no WASI |
| Trust model | "Silo does not sandbox plugins"; installing is the point of trust | Admin grants each Plugin Permission; the kernel checks every call. Also not a sandbox (ADR 0049) |
| Versioning | One `v1` contract; changes must be additive; optional services found by catching `Unimplemented` | Separate semver for each point, refused at load with a plain-words reason (ADR 0043) |
| Distribution | Catalog repo; plugin ID bound to its repo; Sigstore/SLSA signature required for first-party; community catalog; auto-update; version pins; uploads | Bundled in the binary, or dropped into `data_dir/plugins`. **No catalog, signing or update** |
| UI | JSON Schema forms, a `presentation` block, plugin-served pages under `/plugins/{id}/` | Forms from the Manifest; Test button; Personal settings; plugins contribute data only, never served pages (ADR 0058) |
| Languages | Go, through its SDK | Rust PDK; any Extism guest in principle |

**Extension points.**
- **Only in Silo:** auth (OIDC, LDAP), markers, watch-sync (Simkl, MDBList, Floppy), autoscan, network access (Tailscale), plugin HTTP routes, audiobook and ebook backends.
- **Only in Nama:** SubtitleSource, BlobStore, HomeRow, Connector, Recogniser.

**Why Silo chose this.** Go has no stable in-process plugin ABI, so go-plugin over gRPC is the standard choice. Separate processes give crash isolation, memory reclaimed when an idle plugin stops, and resident networking for the overlay plugin. Separate repos and signed releases let a community catalog grow. Silo put its security effort into provenance, not sandboxing.

## 7. Users, auth, identity and social

| Area | Silo | Nama | Better |
|---|---|---|---|
| External sign-in | OIDC and LDAP plugins. The plugin returns typed facts; the host owns accounts, keyed by installation and external subject | local-auth only (ADR 0014 leaves room) | Silo |
| Device or TV sign-in | 8-digit code; also answers Jellyfin Quick Connect | Sign-in Codes | Silo |
| Profiles, PINs, parental controls | Household profiles; ratings resolved to a minimum age (MPAA, BBFC, FSK, ACB…); library and quality caps per profile | None (table stakes T11 and T12) | **Silo** |
| Permissions | Roles, access groups, scoped admin keys | Per-User Permissions in the Seerr style (ADR 0020) | Different; Nama's fits Requests |
| API keys | Account- or profile-scoped (`sa_`) | Plugin API keys only | Silo |
| Discovery and networking | mDNS `_silo._tcp` with identity confirmation; Tailscale plugin; outbound SSRF guard | Caddy or Tailscale recipe; Hub and Relay designed (ADR 0057) | Silo now, Nama on paper |
| Notifications | Durable inbox, then websocket, web push, mobile push through a content-free relay, Discord, HMAC webhooks; plugin channel | ntfy, Discord, Telegram, signed webhook; the kernel picks recipients (ADR 0021) | Silo on reach, Nama on design |
| Watch sync | Simkl and MDBList, with two-way ratings; Trakt retired | None (Trakt planned for M8) | Silo |
| History import | Plex, Jellyfin, Emby; matched by external IDs only | None | **Silo** |
| Recommendations | Per-item embeddings, taste profiles, co-watch, cached rows | Top Picks plugin (genre-weighted) | Silo |
| Multi-server identity | `server_id` only | Hub with federated Accounts and merged Watch State (designed) | Nama on paper |

## 8. Clients, web and engineering

### Clients

**Silo web.**
- React 19, react-router, TanStack Query over a generated OpenAPI client, Radix, Tailwind 4.
- 47 locales from a shared `silo-translations` catalog that also generates the strings for the web, Apple, Android and Roku apps.
- A browser decode engine: FFmpeg AC-3, E-AC-3, DTS and TrueHD compiled to WASM, plus WebCodecs and a WebGPU presenter, so the browser can play files it otherwise can't.
- PGS and ASS subtitles.
- A ⌘K search over every admin setting.

Weakness: very large files (`VideoPlayer.tsx` is 6.1k lines) and a suppression baseline.

**Silo native apps.**
- **Apple:** SwiftUI, about 357k lines, on the in-house AetherEngine player.
- **Android:** Kotlin Multiplatform with Compose and Compose for TV, Media3, about 512k lines, at v0.3.
- **Roku.**

**Nama web.** React 19, TanStack Router, ConnectRPC, shadcn on Base UI, Vite+. Layering is enforced by Fallow, functions fail the CRAP gate at cyclomatic complexity 5, and `jsx-a11y` lint is on. English only, dark mode only, about 42k lines.

The code-quality edge goes to Nama; everything else goes to Silo.

### Engineering and ops

| Area | Silo | Nama |
|---|---|---|
| Tests | ~0.5:1 test lines to source; real Postgres in tests; gates on DB pins, file-size ratchet and migration checks | nextest, structure test (ADR 0051), **flaky-test history and auto-filed issues** (ADRs 0022, 0073) |
| Contract | oasdiff against the merge base, `breaking-approvals.json`, a `LOCKED` file after 1.0 | `buf breaking` plus `proto/BREAKING.md` |
| Decision docs | ~96 architecture docs that state what was decided, plus `non-goals.md` | 76 ADRs that also record rejected options, `GLOSSARY.md`, 160 `AGENTS.md` files |
| AI policy | Disclosure, adversarial review, before/after evidence, PRs auto-closed | Agent-first structure; no contributor policy (none needed yet) |
| Backup | Encrypted, remote destinations, selective restore, keeps the two newest pre-migration copies | tar backup and restore, one pre-upgrade copy, documented rollback (ADR 0068) |
| Support | Consent-gated diagnostics upload, crash ingestion, `logredact`, ops log, Grafana and Prometheus rules, a Claude/Codex troubleshooting skill | OpenTelemetry and a local LGTM stack; no support bundle, no redaction |
| Deploy | Compose, an omnibus image (Postgres, Redis and Meilisearch under s6), Kubernetes, nodes, Unraid templates | One image |
| Releases | No tags yet; `build-N` images; SemVer planned | v0.1.0 via cargo-dist |

## 9. What to borrow, ranked by value per effort

Cheap and high value:

1. ~~**Missing-file grace window**~~. Not filed. #911 already covers the empty-mountpoint case, and Watch State keyed by External ID survives a file leaving and coming back. What remains (the added date, Requests flipping availability) is small.
2. **`If-Range` and a strong ETag on Direct Play** (S, #1363). tower-http's `ServeFile` sends no `ETag` and ignores `If-Range`, despite the comment at `playback/direct_play.rs:67`. Fixing it protects resume when a file is replaced in place (`direct-stream-resume.md`).
3. **Surround to E-AC-3 5.1** (S, #1364). One more planner step and one more ffmpeg argument set. It fixes stereo-only output on Android TV and soundbars.
4. **A non-goals ADR** (S, #1365). State Live TV, IPTV and `.strm` as permanent non-goals, with the reasons written down, as Silo does.
5. ***arr autoscan webhook** (S–M, #1366). A token-authenticated endpoint that scans the exact imported path, de-duplicated and debounced, and never drops a real delivery.
6. ~~**Remember failed plans on replan**~~. Already built: #349 and #351 narrow out a method that failed. Silo's attempt keys are per recipe, which is finer, but nothing is filed for that.
7. **`.nomedia` and `.ignore` files** (S, #1367).
8. **A Manifest `presentation` block** (S, #1368). Markdown setup steps, links, license and changelog for each plugin.

Medium:

9. **Parental controls and profiles** (M). Normalise Content Ratings to a minimum age, then cap per User. This is table stakes T12. Silo's `internal/access/rating.go` table is a good reference.
10. **OIDC as an Installed Plugin** (M). Key the identity by plugin and subject, with the kernel owning sessions (ADR 0014). Treat an unknown account status as unavailable, never as active.
11. **History import from Plex, Jellyfin and Emby** (M). Match by external IDs only; our Watch State key already makes this natural. It lowers the cost of switching.
12. **NFO tier** (M). Precedence: admin locks, then NFO, then remote; remote never extends a list the NFO set.
13. **A signed plugin catalog** (M). A `manifest.json` with the `.wasm` SHA-256 and a Sigstore bundle, the plugin ID bound to its repo, and Install and Update RPCs. A single `.wasm` file makes this simpler than Silo's per-platform binaries.
14. **Watch-sync point** (M). Simkl and Trakt scrobbling, before M8.
15. **Support bundle with log redaction** (M). Have it before the first outside user files a bug.

Large, or strategic, and needing a decision:

16. **Reach: decision D1.** Silo shows both routes working at once: native apps on a typed contract, plus a Jellyfin-compat layer as the on-ramp. Its `jellycompat` package is 218 files, which shows what the shim really costs, and the shim hides Requests. Our recommendation stays: PWA and Media Session now, then one native Android TV app on the generated ConnectRPC client. A Jellyfin shim only as an optional sidecar.
17. **Intro detection by Chromaprint** (L). It fits once ADR 0075's outside sources prove too thin.
18. **HEVC output, DV RPU preflight, trickplay** (M–L). Trickplay is already ADR 0076.
19. **Translation pipeline** (L). One English catalog with generated output, when i18n starts.

Don't borrow:
- Unsandboxed native plugins, or plugin-served HTML pages (ADR 0058 rejects these for good reason).
- Postgres, Redis or a cluster for single-box users.
- Banning media deletion.
- Book, manga or podcast kinds.
- 6k-line component files.

## 10. Where Nama should press its advantage

- **Simplicity as a feature.** Silo needs Postgres tuning commands, `PG_MAJOR` dump-and-restore, Redis and an omnibus s6 image. Nama is one binary and one file. Publish memory, time-to-first-frame and large-library numbers; neither project has them.
- **Plugin safety.** A Silo plugin runs as root with full disk and network access. Nama's are WASM with granted permissions. Make this concrete with a catalog, but don't call it a sandbox (ADR 0049).
- **Requests as the product**, together with coordinated deletes. Silo has Requests but treats the library as read-only. Nama can close the whole loop: request, watch, clean up.
- **The Hub.** Federated identity, merged Watch State across Servers and per-Server TLS are things Silo explicitly doesn't do. This is our clearest differentiator, and it is unbuilt. Prioritising it matters as much as any feature in §9.
