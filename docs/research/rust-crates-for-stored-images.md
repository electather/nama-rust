# Rust crates for stored images

Research for the stored-images work: where the Server keeps images it owns, how an admin uploads them, how they are served with HTTP caching, and how they are resized and converted on request. Checked on 2026-11-11.

## How this was checked

- **Crate facts** (latest version, dates, releases in the last 12 months, downloads, licence, owners) come from the crates.io API, `https://crates.io/api/v1/crates/<name>` and `/owners`, fetched 2026-11-11. "Releases in 12 mo" counts versions published since 2025-11-11 that are not yanked. "Recent" downloads are crates.io's last-90-days figure. Every crate name in a table links to its crates.io page.
- **Capabilities** come from the published source of the exact version named (linked on docs.rs), the crate's README, or its repository.
- **Would `deny.toml` accept it?** was not judged by reading licences. For each candidate a scratch crate outside this repo depended on that one crate, and `cargo deny check licenses advisories sources` (cargo-deny 0.20.2) ran against a copy of this repo's `deny.toml`. "Passes" and "fails" below are that run's result.
- **"New crates"** is how many crates a candidate adds that `Cargo.lock` does not already hold, from `cargo tree -e normal` on x86_64 Linux.
- **Advisories** were looked up through the OSV API (`https://api.osv.dev/v1/query`, ecosystem `crates.io`), which mirrors RustSec, and through cargo-deny's copy of the RustSec database. IDs link to rustsec.org.
- Nothing from crates.io was compiled or run. The only program run was the system ffmpeg, once, to time still-image encodes.

## What this repo allows

- **Licences** (`deny.toml`): `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Unicode-3.0`, `Zlib`, `AGPL-3.0-only`. Three per-crate exceptions exist: `MPL-2.0` for `cbindgen`, `CDLA-Permissive-2.0` for `webpki-roots`, `CC0-1.0` for `notify`. So `MPL-2.0`, `CC0-1.0`, `IJG`, `NCSA`, `BSL-1.0` and `GPL-3.0` are rejected unless an exception is added.
- **Advisories**: any RustSec advisory fails, "unmaintained" included; yanked crates are denied. `README.md` says a PR fails only on an advisory its own `Cargo.lock` adds.
- **Sources**: `unknown-registry` and `unknown-git` are denied, so a crate that is only on git cannot be used.
- **Graph**: `all-features = true` and no `targets`, so cargo-deny also checks dependencies behind any `cfg`, including ones never built.
- **Bans**: `multiple-versions = "warn"`, `wildcards = "deny"`. No crate is banned by name.
- **Lints** (`Cargo.toml`): `unsafe_code = "deny"`. Calling a raw `-sys` crate from the Server needs a reviewed `#[expect(unsafe_code, reason = "...")]`; a safe wrapper crate does not.
- **Release targets** (`dist-workspace.toml`): `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`. A C dependency has to build on all five. The Docker image is `debian:trixie-slim` plus `jellyfin-ffmpeg8`, built from the release's Linux binaries, so it is a glibc build, not a static musl one.
- **Dependency rules in `AGENTS.md`**: `crates/AGENTS.md` only says to load the `rust-crates` skill; `crates/nama-server/AGENTS.md` has no rule about adding dependencies. The root `README.md` checks include `cargo machete` and `cargo deny check bans licenses sources`.
- **Already in `Cargo.lock`**: `axum` 0.8.9, `tower-http` 0.7.1, `connectrpc` 0.9.1, `sha2` 0.11.1, `http` 1.5.0, `httpdate` 1.0.3, `http-range-header` 0.4.2, `mime` 0.3.18, `mime_guess` 2.0.5, `tempfile` 3.27.0, `tokio-util` 0.7.20, `rayon` 1.12.0, `roxmltree` (a direct dependency of the Server), `ring`, `rustls`, `extism-pdk` 1.4.1.
- **Not in `Cargo.lock`**: every image, storage, sniffing, SVG and placeholder candidate below, and also `headers`, `multer`, `chrono`, `aws-lc-rs`, `reqwest`, `paste`.

## Summary

| Need | Recommended | Pure Rust? | Licence | Why |
| --- | --- | --- | --- | --- |
| 1. Blob storage | Own small trait over `tokio::fs`, with `tempfile` for atomic writes. `object_store` if an S3 backend ever lives in the Server itself | Yes | MIT OR Apache-2.0 | Nothing can cross into an Extism plugin, so the seam has to be the Server's own; local files also let `tower-http` serve them |
| 2a. Decode, orient, JPEG and PNG encode | `image` 0.25, features `jpeg`, `png`, `webp`, `gif` only | Yes | MIT OR Apache-2.0 | Passes `deny.toml`, 13 new crates, has limits and EXIF orientation |
| 2b. Resize | `fast_image_resize` 6 with its `image` feature | Yes | MIT OR Apache-2.0 | In its published benchmark, about 5 times faster than `image`'s own resize without SIMD and 13 times or more with AVX2 |
| 2c. Lossy WebP encode | `webp` 0.3, for encoding only (libwebp through `libwebp-sys` 0.9) | **No, C** | MIT OR Apache-2.0 | No mature permissive pure-Rust lossy WebP encoder exists; this one passes `deny.toml`, adds two crates and builds with `cc` alone |
| 2d. AVIF encode | Leave out of the first version; later, the system ffmpeg's `libsvtav1` | n/a | n/a | `ravif` fails `deny.toml` twice today and wants `nasm`; Next.js itself ships WebP only by default |
| 3. Whole solution | None. Assemble crates in-process | n/a | n/a | Every proxy is a second process with libvips or ImageMagick; no maintained axum or tower optimizer crate exists |
| 4. Type and dimensions | `image::ImageReader::with_guessed_format` and `into_dimensions` | Yes | MIT OR Apache-2.0 | Already a dependency after need 2; the sniffer and the decoder cannot disagree |
| 5. SVG | `svg-hush` at upload, plus GitHub's response headers on every SVG | Yes | Apache-2.0 OR MIT | The CSP `sandbox` header is what stops scripts on direct navigation; sanitising is the second layer |
| 6. HTTP caching | `tower-http` `ServeFile` over files on disk; `headers-accept` for `Accept` | Yes | MIT | 0.7 already does ETag, `If-None-Match`, `Last-Modified` and `Range` |
| 7. Uploads | Plain HTTP body on an axum route with `DefaultBodyLimit` | Yes | MIT | A unary RPC buffers the whole message; browsers cannot client-stream |
| 8. Placeholders | `thumbhash` 0.1 | Yes | MIT | Zero dependencies, keeps alpha and aspect ratio; the format is frozen |
| 9. Hashing | `sha2`, already a direct dependency | Yes | MIT OR Apache-2.0 | No reason for a second hash at upload sizes of a few MiB |

## 1. Blob storage abstraction

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`object_store`](https://crates.io/crates/object_store) | 0.14.2, 2026-09-15 | 7 | 108.9M / 30.2M | Apache Arrow ([apache/arrow-rs-object-store](https://github.com/apache/arrow-rs-object-store)) | MIT/Apache-2.0. `fs` only: passes. With `aws`: fails, `webpki-root-certs` is `CDLA-Permissive-2.0` | `fs`: none. `aws`: `aws-lc-rs` (C) | 5 (`fs`), 11 (`fs` + `aws`) |
| [`opendal`](https://crates.io/crates/opendal) | 0.59.5, 2026-10-18 | 11 | 20.3M / 7.1M | Apache OpenDAL ([apache/opendal](https://github.com/apache/opendal)) | Apache-2.0. `services-fs` only: passes. With `services-s3`: fails, `tiny-keccak` is `CC0-1.0` | none | 8 (`fs`), 21 (`fs` + `s3`) |
| [`rust-s3`](https://crates.io/crates/rust-s3) | 0.38.0, 2026-10-04 | 3 | 11.4M / 1.9M | One owner (`durch`) | MIT, but fails: `attohttpc` and `minidom` are `MPL-2.0`, `tiny-keccak` is `CC0-1.0` | OpenSSL by default (`native-tls`) | 37 |
| [`aws-sdk-s3`](https://crates.io/crates/aws-sdk-s3) | 1.159.0, 2026-11-10 | 48 | 108.1M / 30.3M | AWS (`awslabs`) | Apache-2.0. Default features fail on advisories: they pull `h2` 0.3.27, `rustls` 0.21.12 and `rustls-webpki` 0.101.7 | `aws-lc-rs` (C) | 53 |
| [`rusty-s3`](https://crates.io/crates/rusty-s3) | 0.11.0, 2026-10-27 | 6 | 2.0M / 1.2M | Two owners (`paolobarbolini`, `guerinoni`) | BSD-2-Clause: passes | none | 6 |
| `tokio::fs` + [`tempfile`](https://crates.io/crates/tempfile) | 3.27.0, 2026-03-11 | 4 | 984M / 249M | One owner (`Stebalien`) | MIT OR Apache-2.0: passes | none | 0 |

What each one does on a local filesystem:

- **`object_store` `LocalFileSystem`** ([source](https://docs.rs/crate/object_store/0.14.2/source/src/local.rs), [docs](https://docs.rs/object_store/0.14.2/object_store/local/struct.LocalFileSystem.html)).
  - Writes are atomic: data goes to a staging file named `{path}#{suffix}`, then a rename (`PutMode::Overwrite`) or a hard link (`PutMode::Create`, which returns `AlreadyExists`) publishes it. `with_fsync` is off by default.
  - `PutMode::Update`, the compare-and-swap write, returns `NotImplemented` on the local backend. So do `put_opts` and `put_multipart_opts` when any attribute such as a content type is set: the local backend stores no metadata beside the bytes.
  - The ETag is built from inode, size and mtime, not from content. `GetOptions` carries `if_match`, `if_none_match`, `if_modified_since`, `if_unmodified_since` and `range` ([docs](https://docs.rs/object_store/0.14.2/object_store/struct.GetOptions.html)), and the local `get_opts` checks them.
  - `get` returns a stream; `put` takes the whole payload; `put_multipart` streams. Listing and delete exist; `with_automatic_cleanup` removes emptied directories.
  - I/O runs on `spawn_blocking` when called inside tokio.
  - File names matching `.*#\d+` are reserved for staging and are skipped by listing. Symlinks pointing outside the root are followed.
- **`opendal` `Fs`** ([capabilities](https://docs.rs/crate/opendal-service-fs/0.59.5/source/src/backend.rs), [docs](https://docs.rs/crate/opendal-service-fs/0.59.5/source/src/docs.md)). Writes are atomic only when `atomic_write_dir` is configured. It supports `write_with_if_not_exists`, append, list, delete, copy and rename. Its capability set has no conditional read and its source never produces an ETag. User metadata is stored in xattrs, Unix only.
- **`rust-s3`, `aws-sdk-s3`, `rusty-s3`** speak S3 only; none has a local backend. `rusty-s3` is "Sans-IO": it signs requests and parses responses and leaves the HTTP call to the caller ([README](https://crates.io/crates/rusty-s3)). The `rust-s3` README says of its own upkeep that the author's attention "oscillates" ([README](https://crates.io/crates/rust-s3)).
- **`tempfile`**: `NamedTempFile::persist` "will atomically replace" an existing target, cannot cross filesystems, and syncs neither the file nor the directory ([docs](https://docs.rs/tempfile/3.27.0/tempfile/struct.NamedTempFile.html#method.persist)). `persist_noclobber` is the create-only form.

Inside a WASM plugin: none of them. `object_store` compiles its `fs` backend out on `wasm32`, and its docs say its `reqwest` transport does not compile on `wasm32-wasip1`; a caller there must supply its own `HttpConnector` ([docs](https://docs.rs/object_store/0.14.2/object_store/#disabling-reqwest)). An Extism plugin has no tokio runtime and no sockets. Its only network call is the host function `extism_pdk::http::request`, which sends and returns whole bodies ([docs](https://docs.rs/extism-pdk/1.4.1/extism_pdk/http/fn.request.html)). `rusty-s3` is the one candidate whose design fits that: sign in the plugin, send through the host. Whether it compiles to the plugin's WASM target was not tested.

Open advisories: none for `object_store` 0.14 ([RUSTSEC-2024-0358](https://rustsec.org/advisories/RUSTSEC-2024-0358) was fixed in 0.10.2), `opendal`, `rust-s3` or `rusty-s3`. The `aws-sdk-s3` failures cargo-deny reported here are in its legacy default TLS stack: [RUSTSEC-2026-0258](https://rustsec.org/advisories/RUSTSEC-2026-0258) (`h2`), [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346) and [RUSTSEC-2026-0349](https://rustsec.org/advisories/RUSTSEC-2026-0349) (`rustls` 0.21), and four in `rustls-webpki` 0.101.

**Recommendation (mine).** Define the Server's own small blob trait and implement local disk with `tokio::fs`, writing through `tempfile::NamedTempFile::new_in(dir)` then `persist`. Three facts decide it. An S3 plugin cannot implement `ObjectStore` or OpenDAL's `Operator`, so the plugin seam must be the Server's own either way. Plain files on disk can be handed straight to `tower-http`'s `ServeFile`, which already does ETag, `Range` and conditional requests (need 6). And it adds no crate. If the S3 backend ends up inside the Server instead of a plugin, take `object_store` with `default-features = false, features = ["fs", "aws"]`: it has five times OpenDAL's downloads, a real atomic local backend and conditional reads, at the price of one more `CDLA-Permissive-2.0` exception (for `webpki-root-certs`, the same data as the existing `webpki-roots` exception) and `aws-lc-rs`.

## 2. Decode, resize, encode

### Precise answers first

- **Is lossy WebP encoding available in pure Rust?** Not in `image` or `image-webp`. `image` 0.25.10's `WebPEncoder` has one constructor, `new_lossless`, and its docs say "Right now only lossless encoding is supported" and "If you need lossy encoding, you'll have to use libwebp" ([docs](https://docs.rs/image/0.25.10/image/codecs/webp/struct.WebPEncoder.html)). The `image-webp` README says "This crate only supports lossless encoding" ([repository](https://github.com/image-rs/image-webp)); it has had no release since 0.2.4 on 2025-08-27, and none of its open pull requests adds a lossy encoder. Outside image-rs, yes, three young crates do it: `zenwebp` (AGPL-3.0-only or commercial), `gamut-webp` and `webp-rust` (both permissive). See the table.
- **Is AVIF encode fast enough to do on request?** Only at high speed settings and with assembly. `image` encodes AVIF through `ravif` at a default of speed 4, quality 80 ([source](https://docs.rs/crate/image/0.25.10/source/src/codecs/avif/encoder.rs)). The `cavif` README, by the same author, says speeds 1 and 2 are "unbelievably slow" and "Speeds 7 and above degrade compression significantly, and are not recommended" ([README](https://github.com/kornelski/cavif-rs/blob/main/README.md)). `ravif`'s own table marks speed 10 as "20% faster, but also 10% larger files" ([source](https://docs.rs/crate/ravif/0.13.0/source/src/av1encoder.rs)). `image` turns `ravif`'s assembly off unless its `nasm` feature is on, which "requires `nasm` installed" ([README](https://docs.rs/crate/image/0.25.10/source/README.md)). The Next.js docs put it as "AVIF generally takes 50% longer to encode but it compresses 20% smaller compared to WebP" ([docs](https://nextjs.org/docs/app/api-reference/components/image)). No published millisecond benchmark for `ravif` was found, and none was measured here.
- **Is AVIF decode available without a C library?** Yes, since `avif-decode` 3.0.0 (2026-09-20): "Decoding is done by rav1d, a Rust port of dav1d, so there's no C library to build" ([README](https://crates.io/crates/avif-decode)). It fails `deny.toml` three ways: `avif-parse` is `MPL-2.0`, `to_method` (under `rav1d`) is `CC0-1.0`, and `rav1d` depends on `paste`, flagged unmaintained in [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436). On x86 it enables `rav1d`'s `asm` feature, which needs `nasm` at build time ([manifest](https://docs.rs/crate/avif-decode/3.0.0/source/Cargo.toml), [rav1d README](https://crates.io/crates/rav1d)). It does not read embedded ICC profiles or HDR. `image`'s own AVIF decoder still needs C: its `avif-native` feature "Enables non-Rust dependencies of `avif` (`mp4parse` and `dav1d`)", and `mp4parse` is `MPL-2.0`.

### Decoders and all-in-one libraries

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`image`](https://crates.io/crates/image) | 0.25.10, 2026-03-10 | 2 | 247M / 79.5M | image-rs org | MIT OR Apache-2.0. `jpeg`+`png`+`webp`+`gif`: passes. `avif`: fails (see `ravif`). `avif-native`: fails (`mp4parse` MPL-2.0) | none without `avif-native` | 13; 45 with `avif` |
| [`image-webp`](https://crates.io/crates/image-webp) | 0.2.4, 2025-08-27 | 0 | 107M / 45.4M | image-rs (`fintelia`) | MIT OR Apache-2.0: passes | none | with `image` |
| [`zune-jpeg`](https://crates.io/crates/zune-jpeg) | 0.5.15, 2026-03-26 | 13 | 160M / 67.6M | One owner (`etemesi254`) | MIT OR Apache-2.0 OR Zlib: passes | none | with `image` |
| [`zune-image`](https://crates.io/crates/zune-image) | 0.5.0, 2026-01-24 | 1 | 134k / 52k | Same | Fails: its JPEG encoder `jpeg-encoder` is `(MIT OR Apache-2.0) AND IJG` | none | 34 |
| [`rimage`](https://crates.io/crates/rimage) | 0.14.0, 2026-09-27 | 3 | 34k / 0.7k | One owner | Fails, through `zune-image` | none in the minimal build | 35 |
| [`photon-rs`](https://crates.io/crates/photon-rs) | 0.3.3, 2025-05-10 | 0 | 94k / 9k | One owner | Apache-2.0 | not checked | not checked |
| [`libvips`](https://crates.io/crates/libvips) | 2.3.0, 2026-06-19 | 9 | 1.3M / 282k | OLX Group | MIT: passes | **System libvips and glib**, linked dynamically ([build.rs](https://docs.rs/crate/libvips/2.3.0/source/build.rs)) | 2 |
| [`avif-decode`](https://crates.io/crates/avif-decode) | 3.0.0, 2026-09-20 | 3 | 73k / 22k | One owner (`kornelski`) | BSD-3-Clause, but fails (above) | none; `nasm` at build time on x86 | 20 |
| [`dav1d`](https://crates.io/crates/dav1d) | 0.11.1, 2025-11-25 | 1 | 1.7M / 531k | rust-av (`lu-zero`) | MIT: passes | **System libdav1d ≥ 1.3** via pkg-config, or a git clone and meson build ([build.rs](https://docs.rs/crate/dav1d-sys/0.8.3/source/build.rs)) | 9 |
| [`libavif`](https://crates.io/crates/libavif) | 0.14.0, 2024-07-05 | 0 | 101k / 21k | One owner | BSD-2-Clause, but fails (`rav1e` 0.7.1) | **C** (libavif, libdav1d) | 25 |

What `image` 0.25.10 gives this use:

- **Decoders**: JPEG (through `zune-jpeg`), PNG, WebP lossy and lossless (through `image-webp`), GIF. A GIF decodes as its first frame through `ImageDecoder`; the docs do not say so in those words, so check it in a test.
- **Encoders**: JPEG with a 1 to 100 quality, PNG, lossless WebP, AVIF (through `ravif`).
- **Decompression-bomb limits**: `ImageReader` starts from `Limits::default()`, which is `max_alloc` 512 MiB and no width or height limit ([source](https://docs.rs/crate/image/0.25.10/source/src/io/limits.rs)). Only `max_image_width` and `max_image_height` are strict; `max_alloc` "is non-strict by default and some decoders may ignore it" ([docs](https://docs.rs/image/0.25.10/image/struct.Limits.html)). `ImageReader::decode` charges the output buffer against `max_alloc` for every format, but only the PNG, JPEG, TIFF and GIF decoders override `set_limits` to count their internal allocations; the WebP and AVIF decoders do not ([source](https://docs.rs/crate/image/0.25.10/source/src/io/decoder.rs)). So set explicit width and height limits; they are the only ones every decoder enforces.
- **EXIF orientation**: `ImageDecoder::orientation()` reads it and `DynamicImage::apply_orientation` applies it. It is not applied automatically.
- **Colour profiles**: `ImageDecoder::icc_profile()` returns the raw profile. Nothing converts pixels through it for you; `image` depends on `moxcms` and offers CICP-based `convert_color_space`, not ICC transforms. The JPEG, PNG and WebP encoders accept `set_icc_profile`; the AVIF encoder accepts only `set_exif_metadata`.
- **Metadata stripping** is the default: an encoder writes no EXIF, XMP or ICC unless the caller sets it. Stripping the ICC profile from a wide-gamut source changes how it looks, so either carry the profile over or convert to sRGB with `moxcms` first.
- **Async**: all of it is CPU-bound and synchronous. Run it under `tokio::task::spawn_blocking` behind a semaphore.
- **Advisories**: none open. The two on record, [RUSTSEC-2019-0014](https://rustsec.org/advisories/RUSTSEC-2019-0014) and [RUSTSEC-2020-0073](https://rustsec.org/advisories/RUSTSEC-2020-0073), were fixed by 0.23.12.

### Resizers

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`fast_image_resize`](https://crates.io/crates/fast_image_resize) | 6.1.0, 2026-07-21 | 4 | 24.3M / 7.1M | One owner (`Cykooz`) | MIT OR Apache-2.0: passes | none | 3 |
| [`resize`](https://crates.io/crates/resize) | 0.8.9, 2026-02-07 | 1 | 2.3M / 297k | PistonDevelopers, `kornelski` | MIT: passes | none | 2 |
| [`pic-scale`](https://crates.io/crates/pic-scale) | 0.7.12, 2026-09-10 | 15 | 3.7M / 3.3M | One owner (`awxkee`) | BSD-3-Clause OR Apache-2.0: passes | none | 5 |
| `image::imageops` | with `image` | | | | | none | 0 |

`fast_image_resize`'s README benchmark, RGB8 4928x3279 to 852x567, single thread, in milliseconds ([README](https://github.com/Cykooz/fast_image_resize)):

| | Bilinear | Bicubic | Lanczos3 |
| --- | --- | --- | --- |
| `image` | 78.41 | 127.38 | 173.98 |
| `resize` | 48.44 | 92.29 | 137.61 |
| libvips | 5.67 | 9.76 | 16.07 |
| `fast_image_resize`, no SIMD | 15.71 | 26.43 | 37.47 |
| `fast_image_resize`, AVX2 | 3.53 | 7.23 | 13.26 |

It has SSE4.1, AVX2, NEON and WASM SIMD128 paths, an `image` feature that takes `DynamicImage` directly, and a `rayon` feature. Two caveats from the same README: it does not convert to linear light while resizing (it supplies `create_srgb_mapper()` for callers who want that), and "The image crate does not support multiplying and dividing by alpha channel", which `fast_image_resize` does. Its source uses `unsafe` heavily for SIMD (686 occurrences in 6.1.0); no advisory is on record.

### Encoders for the modern formats

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`webp`](https://crates.io/crates/webp) | 0.3.1, 2025-08-29 | 0 | 5.8M / 1.7M | One owner (`jaredforth`) | MIT OR Apache-2.0: passes | **C**: pinned to `libwebp-sys` ^0.9.3, whose vendored headers say libwebp 1.3.1 | 2 |
| [`libwebp-sys`](https://crates.io/crates/libwebp-sys) | 0.14.4, 2026-04-29 | 4 | 7.8M / 2.9M | One owner (`NoXF`) | MIT: passes | **C**, vendored libwebp 1.6.0, `cc` only, no cmake | 1 |
| [`webpx`](https://crates.io/crates/webpx) | 0.4.1, 2026-10-25 | 2; its 13 earlier versions are all yanked | 903k / 433k | Imazen | MIT OR Apache-2.0: passes | **C**: `libwebp-sys` ^0.14.2 | 8 |
| [`zenwebp`](https://crates.io/crates/zenwebp) | 0.4.6, 2026-10-24 | 12; first published 2026-01-24 | 53k / 21k | Imazen | AGPL-3.0-only OR commercial: licence passes; fails on `paste` (through `garb`) | none; `#![forbid(unsafe_code)]` | 15 |
| [`gamut-webp`](https://crates.io/crates/gamut-webp) | 0.3.1, 2026-08-05 | 4 of its 4 | 2.4k / 1.9k | One owner | MIT OR Apache-2.0: passes | none | 8 |
| [`webp-rust`](https://crates.io/crates/webp-rust) | 0.5.0, 2026-11-11 | 8 of its 8 | 31k / 28k | One owner | MIT | none | not checked |
| [`ravif`](https://crates.io/crates/ravif) | 0.13.0, 2026-01-19 | 1 | 60.7M / 20.9M | One owner (`kornelski`) | BSD-3-Clause, but fails twice through `rav1e` | none; `nasm` for x86_64 assembly | 33 |
| [`rav1e`](https://crates.io/crates/rav1e) | 0.8.1, 2025-06-16 | 0 | 60.1M / 20.9M | Xiph (`lu-zero`) | BSD-2-Clause, but see below | none; `nasm` and `cc` for assembly | with `ravif` |
| [`gamut-avif`](https://crates.io/crates/gamut-avif) | 2.0.0, 2026-10-16 | 6 of its 6 | 180k / 94k | One owner | MIT OR Apache-2.0: passes | none | 8 |
| [`mozjpeg`](https://crates.io/crates/mozjpeg) | 0.10.13, 2025-02-18 | 0 | 5.0M / 1.3M | One owner (`kornelski`) | **IJG**: fails | **C**, plus `nasm` on x86 | 4 |
| [`jpeg-encoder`](https://crates.io/crates/jpeg-encoder) | 0.7.1, 2026-07-27 | 2 | 9.9M / 2.7M | One owner | **(MIT OR Apache-2.0) AND IJG**: fails | none | 1 |
| [`oxipng`](https://crates.io/crates/oxipng) | 10.2.1, 2026-09-02 | 5 | 2.4M / 619k | oxipng org | MIT: passes | **C**: `libdeflate-sys` | 4 |

Notes on these:

- **Why `ravif` fails.** `rav1e` 0.8.1 depends on `paste`, which [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436) marks unmaintained, and lists `libfuzzer-sys` (`(MIT OR Apache-2.0) AND NCSA`) under `cfg(fuzzing)`. The second is never compiled, but `deny.toml` sets no `targets`, so cargo-deny still checks it. `rav1e`'s master branch has moved to `pastey` ([manifest](https://github.com/xiph/rav1e/blob/master/Cargo.toml)), but no release carries that; the last release is seventeen months old. Using `ravif` today means an advisory `ignore` and a licence exception.
- **`webp` pins an old libwebp.** `webp` 0.3.1 requires `libwebp-sys` ^0.9.3 ([manifest](https://docs.rs/crate/webp/0.3.1/source/Cargo.toml)), and 0.9.6 is the last of that line (2024-09-02). Its vendored tree does contain the fix for [RUSTSEC-2023-0061](https://rustsec.org/advisories/RUSTSEC-2023-0061), but libwebp 1.6.0 is reachable only through `libwebp-sys` 0.14 or `webpx`. `webp` itself had an unsoundness advisory in its encoder, [RUSTSEC-2024-0443](https://rustsec.org/advisories/RUSTSEC-2024-0443), published 2024-09-06 and fixed in 0.3.1 almost a year later. Its API for this use is `Encoder::from_rgba(bytes, width, height).encode(quality)`; with `default-features = false` it does not depend on `image` at all.
- **`libwebp-sys`** is the best-kept of the three libwebp crates: 33 versions since 2017, none yanked, four in the last year. It is raw FFI, so the Server would hold the `unsafe` itself.
- **`webpx`** is a safe wrapper with decode limits. Its README says it "started as an LLM-authored test harness for zenwebp" and steers new projects to `zenwebp` ([README](https://crates.io/crates/webpx)). It was first published 2026-01-20, and every version before 0.4.0 has since been yanked. With `yanked = "deny"` in `deny.toml`, a crate that yanks this freely can break CI on a day of its choosing.
- **`zenwebp`** claims lossy and lossless encode and decode, no C and no `unsafe` ([README](https://crates.io/crates/zenwebp)). `AGPL-3.0-only` is on the allow-list, though the comment there says the entry is "for this workspace's own crates". It is under ten months old with 53k downloads.
- **`gamut-*`** is one person's clean-room codec family, first published in 2026. `gamut-avif` encodes but leaves AV1 decoding to a plug-in decoder "until the workspace's pure-Rust AV1 decoder lands" ([README](https://crates.io/crates/gamut-avif)). Its WebP README claims every other Rust WebP decoder wraps libwebp, which `image-webp` disproves, so weigh its other claims accordingly.
- **Only encoding with libwebp keeps its risk small.** libwebp's well-known holes, including the one behind RUSTSEC-2023-0061, were in decoding. If uploads are decoded by `image-webp` and libwebp only ever sees pixels the Server produced, that class of bug is out of reach.
- **JPEG XL, note only.** [`jpegxl-rs`](https://crates.io/crates/jpegxl-rs) is `GPL-3.0-or-later` and binds libjxl (C++). [`jxl-oxide`](https://crates.io/crates/jxl-oxide) (MIT OR Apache-2.0) and [`jxl`](https://crates.io/crates/jxl) (BSD-3-Clause, from the libjxl project) are pure-Rust decoders. A pure-Rust lossy encoder was not looked for.
- **EXIF**: [`kamadak-exif`](https://crates.io/crates/kamadak-exif) 0.6.2 (2026-11-05, BSD-2-Clause, passes) is not needed; `image` reads orientation itself.

**Recommendation (mine).** Decode, orient and encode JPEG and PNG with `image` (`default-features = false`, features `jpeg`, `png`, `webp`, `gif`), with explicit `Limits` for width and height. Resize with `fast_image_resize`. For lossy WebP take `webp` 0.3.1 with `default-features = false`, and use it only to encode. libwebp is the one C dependency this need cannot avoid if WebP is made in-process, and `webp` is the least costly way in: two new crates, a safe API, on crates.io since 2019, a build that needs only a C compiler. Its price is a 2023-era libwebp and a slow maintainer, which matters little while it only ever encodes the Server's own pixels. If a current libwebp is wanted, call `libwebp-sys` 0.14 directly from one small module with a reviewed `#[expect(unsafe_code)]`; I would not take `webpx` while it yanks versions at its present rate. Leave AVIF output out of the first version: the pure-Rust encoder fails this repo's cargo-deny twice, needs `nasm` to be fast, and Next.js itself defaults to WebP only. Leave AVIF input out too, for the licence reasons above. Do not use `zune-image`, `rimage`, `mozjpeg` or `jpeg-encoder` (IJG), nor `libvips` (system library).

## 3. Whole-solution alternatives

### Shelling out to the system ffmpeg

`jellyfin-ffmpeg`'s Debian build enables `--enable-libwebp`, `--enable-libsvtav1`, `--enable-libdav1d` and `--enable-libzimg`; it does not enable libaom, librav1e or libjxl ([debian/rules](https://github.com/jellyfin/jellyfin-ffmpeg/blob/jellyfin/debian/rules)). So that build can scale stills, write lossy WebP, write AV1 with SVT-AV1, and write JPEG with ffmpeg's own encoder.

Checked here on this machine's ffmpeg n9.0.2 (an Arch build, not jellyfin-ffmpeg) with a synthetic 3000x4500 PNG; times are wall-clock for the whole process, PNG decode included:

| Output | Command tail | Time |
| --- | --- | --- |
| 640 px WebP | `-vf scale=640:-2 -c:v libwebp -quality 75` | 0.17 s |
| 640 px AVIF | `-vf scale=640:-2 -frames:v 1 -c:v libsvtav1 -preset 8 -crf 30 -pix_fmt yuv420p` | 0.32 s |
| 640 px JPEG | `-vf scale=640:-2 -q:v 4 -frames:v 1 -update 1` | 0.16 s |
| 1920 px WebP | as above | 0.34 s |
| 1920 px AVIF | as above | 0.56 s |

`ffprobe` confirmed the AVIF as AV1 640x960 `yuv420p`. A synthetic image says little about real photos, so read these as "a fraction of a second", nothing finer.

Trade-off: no new crate and no C in the binary, and it is the fastest AVIF route found. Against it: ADR 0026 lets the Server run without ffmpeg, so images would need a fallback; which encoders exist varies by build and must be asked through `Abilities`; each variant costs a process; transparency, EXIF orientation and colour handling through ffmpeg were not checked.

### Image proxies

| Project | What it is | Licence | Needs | Fit for one binary |
| --- | --- | --- | --- | --- |
| [imgproxy](https://github.com/imgproxy/imgproxy) | Go server on libvips; "works as a standalone HTTP server"; Docker recommended | Apache-2.0 | libvips | Sidecar only |
| [imagor](https://github.com/cshum/imagor) | Go server (GitHub lists Go; its use of libvips was not checked) | Apache-2.0 | not checked | Sidecar only |
| [thumbor](https://github.com/thumbor/thumbor) | Python server | MIT | Python runtime | Sidecar only |
| [`pict-rs`](https://crates.io/crates/pict-rs) 0.5.25, 2026-10-21 | Rust server, one owner | AGPL-3.0-only | ImageMagick 7, ffmpeg and exiftool on `PATH` ([README](https://crates.io/crates/pict-rs)) | A whole application with its own database, not a library |
| [imageflow](https://github.com/imazen/imageflow) | Rust library with C parts; a "release candidate" | AGPLv3 or commercial | C toolchain, NASM | `imageflow_core` is not on crates.io, so `unknown-git = "deny"` rejects it; its README says its interfaces "are not stable" |

### An image optimizer crate for axum or tower

None that is maintained at any scale. crates.io searches for "image optimizer", "image proxy", "axum image", "tower image resize", "image resizing server", "imgproxy", "thumbnail axum" and "next image optimizer" ([search](https://crates.io/search?q=imgproxy)) turned up only `truss-image` (834 downloads), `imgforge` (843), `oximg` (749), `axum-image` (745, AGPL-3.0-or-later) and `crabcrop` (96).

**Recommendation (mine).** Embed nothing and run no sidecar. A self-hosted server that ships as one binary would have to ship, start, watch and secure a second process with its own native libraries, to do what `image` plus `fast_image_resize` do in about a hundred lines. Keep the system ffmpeg in mind for one job only: AVIF output later, as an optional extra where `Abilities` finds `libsvtav1`.

## 4. Recognising the real type, and reading dimensions

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `image::guess_format` | with `image` | | | | passes | none | 0 |
| [`infer`](https://crates.io/crates/infer) | 0.22.0, 2026-07-15 | 1 | 145M / 36.3M | One owner (`bojand`) | MIT: passes | none | 1 without default features, 3 with |
| [`file-format`](https://crates.io/crates/file-format) | 0.29.0, 2026-03-27 | 1 | 2.3M / 675k | One owner | MIT/Apache-2.0: passes | none | 1 |
| [`tree_magic_mini`](https://crates.io/crates/tree_magic_mini) | 3.2.2, 2025-11-14 | 2 | 26.8M / 13.7M | One owner (`mbrubeck`) | MIT: passes | **The system's shared-mime-info database at run time**, or GPL data compiled in | 2 |
| [`imagesize`](https://crates.io/crates/imagesize) | 0.15.0, 2026-07-09 | 1 | 47.3M / 20.4M | One owner | MIT: passes | none | 1 |

- **`image::guess_format`** matches magic bytes for PNG, JPEG, GIF, WebP, TIFF, BMP, ICO and others. It knows AVIF only by the `ftypavif` brand and has no SVG entry ([source](https://docs.rs/crate/image/0.25.10/source/src/io/free_functions.rs)). `ImageReader::with_guessed_format()` applies it to a reader.
- **`image::ImageReader::into_dimensions`** builds the decoder and asks it, so it reads headers and not pixels ([source](https://docs.rs/crate/image/0.25.10/source/src/io/image_reader_type.rs)). It works only for formats whose decoder is compiled in.
- **`infer`** has matchers for JPEG, PNG, GIF, WebP, AVIF, HEIF, JPEG XL, TIFF, BMP, ICO and more, and none for SVG ([docs](https://docs.rs/infer/0.22.0/infer/)).
- **`file-format`** lists AVIF, WebP and SVG; its SVG signature is the bytes `<svg` at the start, so an SVG that opens with an XML declaration or a comment needs its `reader-xml` feature ([source](https://docs.rs/crate/file-format/0.29.0/source/src/signatures.rs)).
- **`tree_magic_mini`**: "the magic database files themselves are licensed under the GPL", and without the embedded copy it loads the database "from the standard locations at runtime" ([README](https://crates.io/crates/tree_magic_mini)). Windows and macOS have no such database.
- **`imagesize`** reads dimensions from a slice or reader for JPEG, PNG, GIF, WebP, HEIF/AVIF, JPEG XL and many others, without a decoder ([docs](https://docs.rs/imagesize/0.15.0/imagesize/)).
- No advisory is open on any of the five.

**Recommendation (mine).** Use what need 2 already brings: `ImageReader::new(Cursor::new(bytes)).with_guessed_format()`, then `into_dimensions()` under the same `Limits`. The sniffer then is the decoder's own, so "what we said it is" and "what we can decode" cannot drift apart. Recognise SVG separately by parsing it: `svg-hush` rejects anything that is not well-formed SVG, and `roxmltree` is already a dependency if a cheaper root-element check is wanted. Add `imagesize` only if AVIF dimensions are wanted without an AVIF decoder, and `infer` (`default-features = false`) only if non-image types need naming.

## 5. SVG safety

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`svg-hush`](https://crates.io/crates/svg-hush) | 0.9.7, 2026-08-06 | 2 | 684k / 437k | Cloudflare | Apache-2.0 OR MIT: passes | none | 4 |
| [`ammonia`](https://crates.io/crates/ammonia) | 4.2.4, 2026-10-17 | 16 | 21.5M / 7.6M | rust-ammonia org | MIT OR Apache-2.0 | none | not checked |
| [`resvg`](https://crates.io/crates/resvg) / [`usvg`](https://crates.io/crates/usvg) | 0.50.0, 2026-11-11 | 6 | 37.5M / 15.4M | Linebender | Apache-2.0 OR MIT: passes | none | 39; 19 without default features |
| [`tiny-skia`](https://crates.io/crates/tiny-skia) | 0.12.0, 2026-02-02 | 1 | 68.4M / 26.9M | Linebender | BSD-3-Clause: passes | none | with `resvg` |

- **`svg-hush`** "removes any elements and attributes that aren't in its allowlist and filters all URLs to be same-origin only". Its README is plain about its place: "it's best to serve SVG images with a restrictive `Content-Security-Policy`. This tool is a defense-in-depth". It accepts only well-formed XML in the SVG namespace, in UTF-8, UTF-16 or latin1 ([README](https://github.com/cloudflare/svg-hush)). No advisory.
- **`ammonia`** is not an SVG sanitiser. It parses "a fragment of HTML", and its default tag list holds no `svg` or `path` ([docs](https://docs.rs/ammonia/4.2.4/ammonia/struct.Builder.html)). Its advisories show how hard SVG inside HTML is: three in 2026 alone, [RUSTSEC-2026-0193](https://rustsec.org/advisories/RUSTSEC-2026-0193), [RUSTSEC-2026-0213](https://rustsec.org/advisories/RUSTSEC-2026-0213) (XSS through SVG `animate` and `set`) and [RUSTSEC-2026-0373](https://rustsec.org/advisories/RUSTSEC-2026-0373), all fixed in 4.2.4.
- **`resvg`** renders "the static SVG subset; i.e. no a, script, view or cursor elements, no events and no animations", with "no non-Rust code in the final binary" ([README](https://github.com/linebender/resvg)). The output is a bitmap, so nothing in it can run. Two traps: its default features pull in system fonts and memory-mapped font files, and `usvg`'s default resolver for `<image href>` "treats an input string as a file path and tries to open" it ([source](https://docs.rs/crate/usvg/0.50.0/source/src/parser/image.rs)). When rendering an untrusted SVG, replace `Options::image_href_resolver` so it never reads local files.

What others send with an untrusted SVG:

- **GitHub raw**, observed on `https://raw.githubusercontent.com/twbs/icons/main/icons/alarm.svg` on 2026-11-11:
  - `content-type: image/svg+xml`
  - `content-security-policy: default-src 'none'; style-src 'unsafe-inline'; sandbox`
  - `x-content-type-options: nosniff`
  - `x-frame-options: deny`
  - `cross-origin-resource-policy: cross-origin`
- **Next.js** refuses SVG by default (`dangerouslyAllowSVG: false`). Its built-in defaults are `contentSecurityPolicy: "script-src 'none'; frame-src 'none'; sandbox;"` and `contentDispositionType: 'attachment'` ([source](https://github.com/vercel/next.js/blob/canary/packages/next/src/shared/lib/image-config.ts)). When SVG is allowed, its docs recommend `contentSecurityPolicy: "default-src 'self'; script-src 'none'; sandbox;"` with `contentDispositionType: 'attachment'` ([docs](https://nextjs.org/docs/app/api-reference/components/image)).

**Which approach lets an untrusted SVG be served from the app's own origin without scripts running on direct navigation?** The response header, not the sanitiser. The CSP `sandbox` directive "applies restrictions to a page's actions including preventing popups, preventing the execution of plugins and scripts", and without `allow-same-origin` the resource "is otherwise treated as being from an opaque origin" ([MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy/sandbox)). So a directly opened SVG runs no script and cannot read the app's cookies or storage. `Content-Disposition: attachment` goes further and stops it rendering at all on navigation, while `<img>` still shows it. Rasterising with `resvg` removes the question, at the cost of no longer serving a vector.

**Recommendation (mine).** Do both cheap things. Run every uploaded SVG through `svg-hush` and store the result, and send GitHub's header set with every SVG response: `Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; sandbox` and `X-Content-Type-Options: nosniff`. Never resize an SVG; serve it as it is, as Next.js does. Add `resvg` (`default-features = false`) only if a raster of an SVG is truly needed, such as a placeholder.

## 6. HTTP caching helpers

What `tower-http` 0.7.1's `ServeFile` and `ServeDir` do, from its source:

- **ETag**: new in 0.7.0. "add strong `ETag` support to `ServeDir`, including `If-Match` and `If-None-Match` precondition handling per RFC 9110" ([changelog](https://docs.rs/crate/tower-http/0.7.1/source/CHANGELOG.md)). The tag is built from mtime (seconds and nanoseconds) and size, not from content ([source](https://docs.rs/crate/tower-http/0.7.1/source/src/services/fs/serve_dir/headers.rs)).
- **Conditional requests**: `If-Match`, then `If-Unmodified-Since`, give 412; `If-None-Match`, then `If-Modified-Since`, give 304, in RFC 9110's order. A 304 carries `ETag` and `Last-Modified` ([source](https://docs.rs/crate/tower-http/0.7.1/source/src/services/fs/serve_dir/open_file.rs)).
- **Range**: one range, with `Accept-Ranges: bytes`. A multi-range request gets 416, or the whole file with `ignore_multi_range_requests(true)` (new in 0.7.1). `If-Range` appears nowhere in the `fs` module.
- **Precompressed files**: `precompressed_gzip`, `_br`, `_deflate` and `_zstd` pick a sibling file by `Accept-Encoding` and add `Vary: accept-encoding`.
- **`Cache-Control`** is never set. The caller adds it.
- **Other sources**: 0.7.0 added a `Backend` trait "to make `ServeDir` work with non-filesystem sources (e.g. embedded assets or object storage)", used through `ServeDir::with_backend` ([source](https://docs.rs/crate/tower-http/0.7.1/source/src/services/fs/serve_dir/backend.rs)).
- `ServeFile::new_with_mime` sets the content type instead of guessing it from the extension.

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | New crates |
| --- | --- | --- | --- | --- | --- | --- |
| [`tower-http`](https://crates.io/crates/tower-http) | 0.7.1, 2026-08-31 | 7 | 610M / 215M | tower-rs | MIT: passes | 0 |
| [`headers`](https://crates.io/crates/headers) | 0.4.2, 2026-09-24 | 1 | 179M / 33.8M | hyperium (`seanmonstar`) | MIT: passes | 2 |
| [`axum-extra`](https://crates.io/crates/axum-extra), `typed-header` | 0.12.6, 2026-04-14 | 5 | 60.1M / 15.8M | tokio-rs | MIT: passes | 3, `headers` included |
| [`headers-accept`](https://crates.io/crates/headers-accept) | 0.3.0, 2025-11-22 | 1 | 541k / 149k | One owner (`maxcountryman`) | MIT: passes | 3 |
| [`mediatype`](https://crates.io/crates/mediatype) | 0.23.0, 2026-07-27 | 2 | 8.7M / 2.0M | One owner | MIT | with `headers-accept` |
| [`etag`](https://crates.io/crates/etag) | 4.0.0, 2022-12-21 | 0 | 840k / 141k | One owner | **BSL-1.0**: not on the allow-list | not checked |

- **`headers`** has typed `ETag`, `IfNoneMatch` (with `precondition_passes(&ETag)`), `IfMatch`, `IfModifiedSince`, `IfRange`, `LastModified`, `CacheControl`, `Vary`, `Range` and `ContentRange`. It has no `Accept` type ([docs](https://docs.rs/headers/0.4.2/headers/)).
- **`headers-accept`** adds an `Accept` header type that orders media types by precedence and offers `Accept::negotiate` against a list of available types ([docs](https://docs.rs/headers-accept/0.3.0/headers_accept/)). It does not write `Vary`.
- No crate was found that does both the negotiation and the `Vary` for a response.
- For reference, Next.js's optimizer sets `Vary: Accept`, `Cache-Control: public, max-age=<ttl>, must-revalidate`, an ETag it answers 304 for, `Content-Disposition` and `Content-Security-Policy` on every image ([source](https://github.com/vercel/next.js/blob/canary/packages/next/src/server/image-optimizer.ts)). Its format rule: "If the `Accept` header matches more than one of the configured formats, the first match in the array is used", and an animated source keeps its original format.

**Recommendation (mine).** Keep originals and generated variants as files and serve each through `ServeFile::new_with_mime`. That gives ETag, 304, `Last-Modified` and `Range` from a crate already in the tree, and leaves three headers to write by hand: `Cache-Control`, `Vary: Accept`, and the SVG headers from need 5. With content-addressed file names the response can be `immutable`. For choosing a format from `Accept`, `headers-accept` is the maintained option if q-values should be honoured; checking whether `image/webp` appears is all Next.js's rule needs. `headers` is only needed if a response is ever built without a file behind it.

## 7. Uploads

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | New crates |
| --- | --- | --- | --- | --- | --- | --- |
| [`axum`](https://crates.io/crates/axum), `multipart` | 0.8.9, 2026-04-14 | 3 | 584M / 160M | tokio-rs | MIT: passes | 1 (`multer`) |
| [`multer`](https://crates.io/crates/multer) | 3.1.0, 2024-05-04 | 0 | 135M / 33.2M | rwf2 (`SergioBenitez`) | MIT: passes | |
| [`axum_typed_multipart`](https://crates.io/crates/axum_typed_multipart) | 0.17.0, 2026-11-04 | 5 | 5.7M / 1.7M | One owner | MIT | not checked |
| `tower-http`, `limit` | 0.7.1 | | | tower-rs | MIT: passes | 0 |
| [`connectrpc`](https://crates.io/crates/connectrpc) | 0.9.1, 2026-09-21 | 18 | 9.6M / 4.0M | Buf (`bufdev`) | Apache-2.0: in tree | 0 |

- **axum `Multipart`** is built on `multer` and "limits the request body size to 2MB" by default. Fields can be read whole or streamed chunk by chunk ([docs](https://docs.rs/axum/0.8.9/axum/extract/struct.Multipart.html)). `multer` has had no release or push since 2024-05-04; no advisory is on record.
- **`DefaultBodyLimit`**: `Bytes` and the extractors built on it refuse bodies over 2 MB unless `DefaultBodyLimit::max(n)` is layered on the route; it "can also be selectively applied to have different limits for different routes". It does not apply to a handler that reads the body stream itself ([docs](https://docs.rs/axum/0.8.9/axum/extract/struct.DefaultBodyLimit.html)).
- **`tower_http::limit::RequestBodyLimitLayer`** applies to every request whatever reads it, and answers `413 Payload Too Large` from `Content-Length` alone before the body is read ([docs](https://docs.rs/tower-http/0.7.1/tower_http/limit/index.html)).
- **ConnectRPC.** In `connectrpc` 0.9.1, `max_message_size` and `max_request_body_size` both default to 4 MiB (`4 * 1024 * 1024`). For a unary call "the body is read in full before processing". One route can carry its own limits through `Router::with_route_limits`, so "an upload-shaped method can exceed the default without raising it for the rest" ([docs](https://docs.rs/connectrpc/0.9.1/connectrpc/struct.Limits.html)). Client-streaming calls are read message by message, but the browser's fetch transport cannot make them ([connect-es issue 669](https://github.com/connectrpc/connect-es/issues/669)). The Protocol Buffers guide says "Protobuf is not designed to handle large messages" and gives "larger than a megabyte each" as the point to "consider an alternate strategy" ([guide](https://protobuf.dev/programming-guides/techniques/)). The locked 0.9.1 is the release that fixed [RUSTSEC-2026-0304](https://rustsec.org/advisories/RUSTSEC-2026-0304).

**Recommendation (mine).** Upload over plain HTTP: one axum route that takes the image as the request body, with `DefaultBodyLimit::max` set for that route and the type read from the bytes (need 4), not from `Content-Type`. That adds no crate, never holds more than the limit, and returns an id the RPCs can then name. Turn on axum's `multipart` feature only if the web app has to send a form. Do not put image bytes in a unary RPC: it works up to 4 MiB, but the whole message is held in memory at least twice (the body, then the decoded field), and it sits above the size protobuf's own guide draws the line at.

## 8. Placeholders

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Pure Rust | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`blurhash`](https://crates.io/crates/blurhash) | 0.2.3, 2024-07-23 | 0 | 1.3M / 505k | Whisperfish | Apache-2.0/MIT: passes | Yes, no `unsafe` | 1 without default features |
| [`thumbhash`](https://crates.io/crates/thumbhash) | 0.1.0, 2023-03-22 | 0 | 698k / 311k | One owner (`evanw`, the format's author) | MIT: passes | Yes, no `unsafe`, no dependencies | 1 |
| [`fast-blurhash`](https://crates.io/crates/fast-blurhash) | 1.0.1, 2023-03-05 | 0 | 18k / 4k | One owner | Apache-2.0 | not checked | not checked |

- **`blurhash`** encodes and decodes; optional `image` and `gdk-pixbuf` features. Its repository was last pushed 2025-12-17. One past advisory, [RUSTSEC-2023-0083](https://rustsec.org/advisories/RUSTSEC-2023-0083) (a panic on crafted input), fixed in 0.2.0.
- **`thumbhash`** is a single file with four functions: RGBA to hash, hash to RGBA, average colour, and approximate aspect ratio ([source](https://docs.rs/crate/thumbhash/0.1.0/source/src/lib.rs)). It keeps the alpha channel. It has had one release ever, and its repository was last pushed 2024-05-26. No advisory.
- Neither is actively developed. Both implement a fixed, small algorithm, so "finished" describes them better than "abandoned", but nobody is on call for either.

**Recommendation (mine).** `thumbhash`. It adds one crate with no dependencies, keeps transparency and the aspect ratio, and its author also publishes the JavaScript decoder the web app would use. Take `blurhash` instead only if a client that already speaks BlurHash has to be served.

## 9. Content addressing and hashing

| Crate | Latest | Releases in 12 mo | Downloads, all / recent | Maintainers | Licence and `deny.toml` | Native dependencies | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| [`sha2`](https://crates.io/crates/sha2) | 0.11.1, 2026-10-12 | 4 | 1,209M / 373M | RustCrypto | MIT OR Apache-2.0: in tree | none | 0 |
| [`blake3`](https://crates.io/crates/blake3) | 1.8.7, 2026-08-20 | 5 | 225M / 61.3M | BLAKE3 team | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception: passes | **C and assembly through `cc`**, unless the `pure` feature is on ([manifest](https://docs.rs/crate/blake3/1.8.7/source/Cargo.toml)) | 3 |

`sha2` is already a direct dependency of the Server at 0.11, with no open advisory. `blake3`'s case is speed on large inputs, and by default it brings a C build.

**Recommendation (mine).** `sha2`. An upload is a few MiB and is hashed once. If variants are served through `ServeFile`, its own mtime-and-size ETag is enough for them, and the SHA-256 serves as the stored name of the original.

## Findings that differ from a year-old memory

- **`tower-http` now does ETags.** 0.7.0 added strong ETags, `If-Match` and `If-None-Match` to `ServeDir` and `ServeFile`, and a `Backend` trait for non-filesystem sources. In 0.6 neither existed.
- **AVIF can be decoded without C**, through `avif-decode` 3.0 on `rav1d`. It still fails this repo's licence and advisory checks.
- **`image` still cannot write lossy WebP**, and `image-webp` has not released in fourteen months. Pure-Rust lossy WebP encoders now exist, but only in young crates outside image-rs, the most complete of them AGPL or commercial.
- **`image`'s `avif` feature fails this repo's cargo-deny**, because `rav1e` has not released since 2025-06 and still depends on the unmaintained `paste`.
- **The `webp` crate is stuck on a 2023-era libwebp.** Current libwebp (1.6.0) needs `libwebp-sys` 0.14 or `webpx`, and `webpx` has yanked 13 of its 15 versions in ten months.
- **`jpeg-encoder` now declares `IJG`** in its licence, which takes `zune-image` and `rimage` off the table here.
- **`object_store` left the arrow-rs repository** for `apache/arrow-rs-object-store` and is at 0.14, edition 2024. **OpenDAL split** into `opendal-core` and one crate per service.
- **`resvg`, `usvg` and `tiny-skia` moved to Linebender.** `resvg` 0.50.0 was published the day of this research.
- **`connectrpc` has per-route limits** (`Router::with_route_limits`), so one upload-shaped RPC can be raised without raising the rest.
- **`aws-sdk-s3`'s default features** pull a legacy TLS stack that carries seven open advisories.

## Not verified

- No crate was compiled, so "builds on all five release targets" is inferred from each crate's build script and README, not tested. That matters most for `webp` and `libwebp-sys` on `x86_64-pc-windows-msvc` and the two macOS targets.
- Whether any libwebp encoder fix made after the tree that `libwebp-sys` 0.9.6 vendors matters for encoding the Server's own pixels.
- No AVIF or WebP encode through a Rust crate was timed. The only measured times are the system ffmpeg's, on a synthetic image.
- jellyfin-ffmpeg itself was not run. Its codec list comes from its `debian/rules`; the timing came from a different ffmpeg build.
- How ffmpeg handles transparency, EXIF orientation and colour profiles when writing WebP or AVIF stills.
- That `image`'s GIF decoder returns the first frame through `ImageDecoder`. The API implies it; the docs do not state it.
- Whether `rusty-s3` compiles for, and works inside, an Extism plugin.
- The dependency counts and cargo-deny results for `photon-rs`, `webp-rust`, `ammonia`, `axum_typed_multipart`, `fast-blurhash` and `etag`.
- That imagor uses libvips, and thumbor's native dependencies. Only their language and licence were read, from the GitHub API.
- rustsec.org pages were not opened one by one. Advisory data came from the OSV mirror and from cargo-deny's copy of the RustSec database.
- Whether the web client's Connect transport sends `bytes` fields as base64 JSON or as binary, which changes the real ceiling under a 4 MiB message limit.
- How Next.js parses `Accept` internally (substring match or a full negotiation). Only its documented rule and the headers it sets were read.
- The cargo-deny runs covered `licenses`, `advisories` and `sources`. `bans` was not run; in this repo it only warns on duplicate versions.
