# JPEG XL as the served image format

Research for the on-request image resizer: can JPEG XL (JXL) be the main format the Server sends for posters and backdrops, and does its progressive decoding make a thumbhash placeholder unnecessary. Checked on 2026-11-11. It follows up `docs/research/rust-crates-for-stored-images.md`, which planned WebP with a JPEG fallback.

## Summary

1. **Yes, as the first choice, not as the only format.** Chrome 155+ (2026-10-06), Firefox 158+ (2026-10-13) and Safari 17+ (2023-09-18) decode `image/jxl` by default and name it in the `Accept` header of image requests. caniuse counts 54% of global usage with full support and 15% with partial support (Safari) today.
2. **A fallback stays mandatory.** Edge does not decode JXL in any version, nor do Samsung Internet, smart-TV web runtimes (Tizen on Chromium M130, webOS on Chromium 132), Android's platform decoder or Kodi's own ffmpeg build. So: negotiate on `Accept`, send `Vary: Accept`, keep WebP or JPEG.
3. **No encoder is free of cost under this repo's rules.** The reference encoder (libjxl) is reachable through `jpegxl-rs`, which is `GPL-3.0-or-later` and fails `deny.toml`, or through `jxl-sys`, which is raw FFI and needs `unsafe` in the Server; both need cmake and a C++ toolchain on all five release targets. Two pure-Rust lossy encoders now pass `cargo deny` as they are, `jixel` (BSD-3-Clause OR Apache-2.0) and `jxl-encoder` (AGPL-3.0-only OR commercial), but both are under a year old. Section 11, added on 2026-11-12, measures both: each passes every check that could be run here, and `jixel` needs a third of the memory.
4. **The ffmpeg route is closed.** jellyfin-ffmpeg8 is built without libjxl, and ffmpeg's libjxl wrapper has no option that writes a progressive file.
5. **Speed is not the obstacle.** Measured with libjxl 0.12.0 on one thread: effort 3 to 4 encodes 640x960 in 17 to 19 ms and 1920x1080 in 40 to 44 ms, faster than libwebp at its default method (29 and 92 ms). Effort 7 costs 142 and 442 ms.
6. **Smaller files are not a given at poster sizes.** The JPEG XL authors' own summary of subjective tests has JXL about 20 to 30% ahead of WebP on photographs and level with WebP on illustrations and text. The local test here was inconclusive: VMAF put JXL about 27 to 30% under WebP, SSIM and XPSNR put it above.
7. **Progressive decoding gives a placeholder only in Chrome and Firefox, only from files encoded with progressive DC, and only once bytes are flowing.** Measured in Firefox 160: such a file paints a blurred full frame after 0.4 to 2 KB (1 to 5% of the file); a default file first paints at 20 to 60%. It costs 4 to 9% more bytes. Safari paints nothing until the file is complete.
8. **Keep the thumbhash.** It is the only placeholder that exists before the image request is answered, which on a cold variant means while the Server is still decoding, resizing and encoding, and the only one that Safari, Edge and every fallback client get.

## Browser table

Current stable on 2026-11-11: Chrome 157 on desktop, Android and WebView ([chromiumdash](https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=2)), Firefox 160 on desktop and Android ([product-details](https://product-details.mozilla.org/1.0/firefox_versions.json), [mobile](https://product-details.mozilla.org/1.0/mobile_versions.json)), Edge 157 ([release notes](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/release-notes/157)), Safari 27 (per [caniuse](https://caniuse.com/jpegxl); not checked against Apple).

| Browser | Decodes `image/jxl` by default | Since | `image/jxl` in the image `Accept` header | Paints partial data |
| --- | --- | --- | --- | --- |
| Chrome, desktop | Yes | 155, stable 2026-10-06. Behind `chrome://flags/#enable-jxl-image-format` from 145 to 154 | Yes, listed first | Yes for still images, per source; not tested here |
| Chrome, Android | Yes | 155 | Yes (same code) | Same code; not tested |
| Android WebView | Yes, per chromestatus and MDN data | 155 | Same code; not tested | Not tested |
| Chrome, iOS | As Safari on iOS (it renders with WebKit; not verified beyond that) | iOS 17 | As Safari | No |
| Edge, desktop and Android | **No** | Never | Not verified; a secondary report says no | No |
| Firefox, desktop | Yes | 158, 2026-10-13. Nightly-only pref before; Firefox Labs opt-in from 152 | Yes, second after `image/avif` | Yes for still images; measured here in 160 |
| Firefox, Android | Yes, per MDN data and Mozilla's intent ("all platforms") | 158 | Same code; not tested | Not tested |
| Safari, macOS | Yes, still images only | 17.0 (2023-09-26 on macOS) | Yes, third after `image/webp` and `image/avif`; not in Lockdown Mode | **No** |
| Safari, iOS and iPadOS | Yes, still images only | 17.0, 2023-09-18 | Same | **No** |
| Samsung Internet | No, through 31 (caniuse) | | | |

Sources for the table are in section 1.

## How this was checked

- **Browsers.** Chromium source was read as raw files at release tags, Firefox on its `release` branch (160.0.1) and WebKit on `main`; chromestatus, chromiumdash, Mozilla product-details, MDN browser-compat-data, web-features and caniuse were read as JSON or YAML. caniuse is a cross-check, and is named where used.
- **Crates.** As in the earlier survey: crates.io API (`/api/v1/crates/<name>` and `/owners`) fetched 2026-11-11; "releases in 12 mo" counts versions published since 2025-11-11 that are not yanked; capabilities from the published source of the exact version named; advisories from the OSV API, which mirrors RustSec, and from cargo-deny's copy of the RustSec database.
- **`deny.toml` verdicts** are results, not readings: one scratch crate per candidate, outside this repo, depending on that one crate, checked with `cargo deny check licenses advisories sources` (cargo-deny 0.20.2) against a copy of this repo's `deny.toml`. "New crates" is what `cargo tree -e normal` (x86_64 Linux) adds beyond `Cargo.lock`; build-only dependencies such as `cmake` and `bindgen` are not in that count.
- **Nothing from crates.io was compiled or run.** Encoder measurements are of libjxl 0.12.0 itself, through the `cjxl` already installed on this machine.
- **Measurements** ran on an AMD Ryzen 9 7950X3D (16 cores, 32 threads), Linux, with tools already installed: libjxl 0.12.0 (`cjxl`, `djxl`), libwebp 1.6.0 through Pillow 13.0.0, ffmpeg n9.0.2 (an Arch build with `--enable-libjxl`, not jellyfin-ffmpeg), libvips 8.18.7 and Firefox 160.0. The test images are ten system wallpapers (illustrations, 3D renders and graphic art with logos and text; no film stills), centre-cropped to 2:3 and scaled to 640x960 and 300x450 ("poster"), and scaled to 1920x1080 ("backdrop"). Read every number as an order of magnitude for this kind of content, not as a benchmark of real posters.
- **Prose pages** (blog posts, vendor docs, the Mozilla thread, bug pages) were read through a fetch tool that returns an extract; quotes from them are as that tool returned them. The blink-dev thread, the Safari 17 post and the JPEG XL overview paper were read as raw HTML.

## 1. Browser decode support today

**Chrome and Chromium.**

- Chrome 155 turned the feature on. In the 155.0.8059.86 tag, `kJXLImageFormat` is `FEATURE_ENABLED_BY_DEFAULT`; in 154.0.8037.100, and in the 145 and 148 tags, it is `FEATURE_DISABLED_BY_DEFAULT` ([155](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.86/third_party/blink/common/features.cc), [154](https://chromium.googlesource.com/chromium/src/+/refs/tags/154.0.8037.100/third_party/blink/common/features.cc)).
- Chrome 155's stable date was 2026-10-06 ([chromiumdash](https://chromiumdash.appspot.com/fetch_milestone_schedule?mstone=155)). Chrome's own post of the same day says it is "shipping decoding support for the JPEG XL (`.jxl`) image format starting from Chrome 155" ([developer.chrome.com](https://developer.chrome.com/blog/jpeg-xl-in-chrome)). The 155 beta post lists Android, ChromeOS, Linux, macOS and Windows ([beta post](https://developer.chrome.com/blog/chrome-155-beta)).
- chromestatus gives milestone 155 for desktop, Android, WebView and iOS, with the rollout plan "Will ship enabled for all users", the flag `enable-jxl-image-format` and the Finch feature `JXLImageFormat` ([chromestatus](https://chromestatus.com/feature/5114042131808256)). The Intent to Ship of 2026-08-24 got three LGTMs the same day ([blink-dev](https://groups.google.com/a/chromium.org/d/msgid/blink-dev/6a8c2618.f0bac957.7b6f1.087d.GAE%40google.com)).
- The decoder is `jxl-rs`, the Rust decoder from the libjxl project. Chromium 157 vendors the `jxl` crate at 0.7.4 ([README.chromium](https://chromium.googlesource.com/chromium/src/+/refs/tags/157.0.8103.51/third_party/rust/jxl/v0_7/README.chromium)).
- `Accept` for image requests, when the feature is on, is `image/jxl,image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8` ([network_utils.cc](https://chromium.googlesource.com/chromium/src/+/refs/tags/157.0.8103.51/third_party/blink/common/loader/network_utils.cc)). MDN's page of default `Accept` values still shows the older Chrome value without `image/jxl` ([MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Content_negotiation/List_of_default_Accept_values)).
- The decoder is a build option: `enable_jxl_decoder = true`, with the comment "Downstream builds can set this to false to exclude JXL code" ([config.gni](https://chromium.googlesource.com/chromium/src/+/refs/tags/157.0.8103.51/third_party/blink/renderer/config.gni)). So "based on Chromium 155" does not by itself mean a browser decodes JXL.
- The flag entry expires at milestone 158 ([flag-metadata.json](https://chromium.googlesource.com/chromium/src/+/refs/tags/157.0.8103.51/chrome/browser/flag-metadata.json)).

**Edge.** No.

- MDN browser-compat-data has `"edge": { "version_added": false }` for `image/jxl`, while Chrome is `155` ([BCD](https://github.com/mdn/browser-compat-data/blob/main/mediatypes/image/jxl.json)). web-features therefore marks JPEG XL `baseline: false` ([web-features](https://github.com/web-platform-dx/web-features/blob/main/features/jpegxl.yml.dist)).
- Microsoft's web platform release notes for Edge 155 (2026-10-08), 156 and 157 have no JPEG XL entry ([155](https://learn.microsoft.com/en-us/microsoft-edge/web-platform/release-notes/155)). The 155 page first had one: its history holds a commit of 2026-10-09 titled "remove jxl from 155 release notes (#3973)" ([commit](https://github.com/MicrosoftDocs/edge-developer/commit/15f6cd0558), listed by the [commits API](https://api.github.com/repos/MicrosoftDocs/edge-developer/commits?path=microsoft-edge/web-platform/release-notes/155.md)).
- A secondary report of the same day found a JXL test page failing in Edge 155 and loading in Chrome 155 ([PiunikaWeb](https://piunikaweb.com/2026/10/09/microsoft-edge-155-jpeg-xl-support/)). No statement from Microsoft was found.

**Firefox.**

- The 158.0 release notes (2026-10-13): "Firefox now supports the JPEG XL image format" ([release notes](https://www.firefox.com/en-US/firefox/158.0/releasenotes/)).
- On the release branch (160.0.1), `image.jxl.enabled` is `true` ([StaticPrefList.yaml](https://github.com/mozilla-firefox/firefox/blob/release/modules/libpref/init/StaticPrefList.yaml)), and the build option is `--disable-jxl`, so JXL is compiled in unless a build turns it off ([moz.configure](https://github.com/mozilla-firefox/firefox/blob/release/toolkit/moz.configure)).
- `Accept` for image requests is built as `image/avif,image/jxl,image/webp,image/png,image/svg+xml,image/*;q=0.8,*/*;q=0.5` ([nsHttpHandler.cpp](https://github.com/mozilla-firefox/firefox/blob/release/netwerk/protocol/http/nsHttpHandler.cpp)); MDN gives the same value for "Firefox 158 and later" ([MDN](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/Content_negotiation/List_of_default_Accept_values)).
- Mozilla's intent to ship (2026-08-24) targets "all platforms", names `jxl-rs` as the decoder, and moved from 157 to 158 on 2026-09-03 ([dev-platform](https://groups.google.com/a/mozilla.org/g/dev-platform/c/3YMV4MS34KA)). Firefox 160 vendors `jxl` 0.8.0 ([Cargo.lock](https://github.com/mozilla-firefox/firefox/blob/release/Cargo.lock)).
- Checked here: Firefox 160.0 with a fresh profile, no pref changed, decoded every JXL test file.

**Safari.**

- "Safari 17.0 adds support for JPEG XL", and it "is supported by WebKit for Safari 17.0, Safari View Controller and WKWebView on iOS 17, iPadOS 17, watchOS 10, macOS Sonoma, macOS Ventura and macOS Monterey" ([webkit.org](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/), 2023-09-18).
- MDN data notes for Safari 17: "Animated images are not supported" ([BCD](https://github.com/mdn/browser-compat-data/blob/main/mediatypes/image/jxl.json)). caniuse marks every Safari from 17.0 to the Technology Preview as partial: no animation, no progressive decoding ([caniuse data](https://github.com/Fyrd/caniuse/blob/main/features-json/jpegxl.json)).
- WebKit builds the image `Accept` header as `image/webp,image/avif,image/jxl,image/heic,image/heic-sequence,` and more when JPEG XL is compiled in. In Lockdown Mode that prefix is only `image/webp,` ([CachedResourceRequest.cpp](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/loader/cache/CachedResourceRequest.cpp)).

**Negotiating.** None of the three puts a q-value on `image/jxl`, `image/avif` or `image/webp`, and each lists them in a different order. The header says what a client can decode, not what it prefers; the Server has to hold its own preference order.

**Share.** caniuse's data file, updated 2026-11-11, gives 53.99% of global usage with full support and 15.26% with partial support, 69.25% together, out of 97.57% it tracks ([caniuse](https://caniuse.com/jpegxl)). Broken down from caniuse's per-version usage figures (my arithmetic):

| Browser | Share | Receives JXL |
| --- | --- | --- |
| Chrome for Android | 44.02% | 44.02% |
| Chrome, desktop | 24.96% | 8.88% (the rest was still on versions before 155 when the usage data was taken) |
| Safari, iOS | 13.23% | 12.63% (no progressive painting) |
| Edge | 5.05% | 0 |
| Safari, macOS | 2.91% | 2.63% (no progressive painting) |
| Firefox, desktop | 2.13% | 0.69% |
| Samsung Internet | 1.94% | 0 |
| Opera Mobile, Opera, UC, others | about 3% | 0 in this data |

Chrome updates itself, so the Chrome desktop row should close within weeks; that would put the total near 85%. A self-hosted server's own users will differ from a global average, in either direction.

## 2. Progressive rendering

### What each browser does

- **Chrome.** The Blink decoder calls `flush_pixels` whenever the Rust decoder asks for more input, and marks the frame partial: "We may already be able to output some pixel data before we reach the first visible frame". It never flushes animations ([jxl_image_decoder.cc](https://chromium.googlesource.com/chromium/src/+/refs/tags/157.0.8103.51/third_party/blink/renderer/platform/image-decoders/jxl/jxl_image_decoder.cc)). Mozilla describes Blink's implementation as "including animation and progressive display" ([dev-platform](https://groups.google.com/a/mozilla.org/g/dev-platform/c/3YMV4MS34KA)). Not tested here: no Chrome is installed on this machine.
- **Firefox.** `nsJXLDecoder` calls `FlushPartialFrame` when the decoder is about to wait for bytes, for still images only, and allocates its buffer early, which "is what lets flush_pixels render an LF-frame preview into the buffer while we are still waiting on bytes" ([nsJXLDecoder.cpp](https://github.com/mozilla-firefox/firefox/blob/release/image/decoders/nsJXLDecoder.cpp)). Measured below.
- **Safari.** No. WebKit bug 272350, "JPEG XL: Progressive decoding", is still NEW; its latest comment (2026-10-21) reads "Both Firefox and Chrome have shipped JPEG XL support including progressive rendering" ([bug](https://bugs.webkit.org/show_bug.cgi?id=272350)). Mozilla's intent says "Safari has neither progressive rendering nor animation" ([dev-platform](https://groups.google.com/a/mozilla.org/g/dev-platform/c/3YMV4MS34KA)).
- What "progressive rendering" must mean across browsers is not yet specified: Interop 2026 has JPEG XL as an investigation effort, "including defining requirements for progressive rendering" ([web.dev](https://web.dev/blog/interop-2026)).

### What the encoder must do

The format always stores a 1:8 "LF image" before the detail: "there is always a basic progressive preview available in VarDCT mode" ([format overview](https://github.com/libjxl/libjxl/blob/main/doc/format_overview.md)). In practice that is not enough. Firefox 160 first painted default files when 20 to 60% of the bytes had arrived, and painted nothing at 15 to 50% (table below). What makes the early paint is **progressive DC**: the LF image coded as its own frame ahead of the main frame.

- libjxl: `JXL_ENC_FRAME_SETTING_PROGRESSIVE_DC`, "1 to have an extra 64x64 lower resolution pass, 2 to have a 512x512 and 64x64 lower resolution pass" ([encode.h](https://github.com/libjxl/libjxl/blob/v0.12.0/lib/include/jxl/encode.h)). On the command line, `cjxl --progressive_dc=1`.
- `cjxl -p` in libjxl 0.12 sets progressive DC to 1, turns on progressive AC and sets `responsive` ([cjxl_main.cc](https://github.com/libjxl/libjxl/blob/v0.12.0/tools/cjxl_main.cc)). That is new in 0.12: "The progressive flag `-p` in cjxl will now encode a more progressive image" ([changelog](https://github.com/libjxl/libjxl/blob/main/CHANGELOG.md)). With cjxl 0.11.1, `--progressive` alone gave no render until about 39% of a test file, and `--progressive_dc 1` had to be added by hand ([Jake Archibald, 2025-10-15](https://jakearchibald.com/2025/present-and-future-of-progressive-image-rendering/)). Debian trixie packages libjxl 0.11.2 ([sources.debian.org](https://sources.debian.org/src/jpeg-xl/)).
- ffmpeg's libjxl encoder cannot ask for it (section 5), so its files behave like default files.

### Measured: Firefox 160, bytes needed before anything is painted

A local server sent the first N bytes of a file and then stalled; a headless Firefox 160.0 with a throwaway profile was screenshotted about a second later. Files are `cjxl -q 80 -e 4` (libjxl 0.12.0) of three of the test images. "Painted" means the whole image box was covered; in every JXL case the first paint covered the whole frame at once.

| File | Sizes of the three files | Largest prefix that painted nothing | Smallest prefix tested that painted |
| --- | --- | --- | --- |
| 300x450, default | 8.6 to 26.0 KB | 15 to 30% (2.6 to 3.9 KB) | 20 to 40% (3.4 to 5.6 KB) |
| 300x450, `-p` | 10.0 to 27.7 KB | 1 to 3% (0.2 to 0.3 KB) | 2 to 5% (0.4 to 0.6 KB) |
| 640x960, default | 20.4 to 69.0 KB | 20 to 40% (8.1 to 13.8 KB) | 30 to 50% (10.2 to 20.7 KB) |
| 640x960, `-p` | 23.5 to 74.5 KB | 1 to 2% (0.5 KB) | 1 to 3% (0.7 to 1.0 KB) |
| 640x960, `--progressive_dc=1` only | 22.6 to 71.6 KB | same as `-p` | same as `-p` |
| 640x960, ffmpeg `-c:v libjxl` | 20.4 to 68.9 KB | same as default | same as default |
| 1920x1080, default | 40.1 to 182.2 KB | 20 to 50% (20.1 to 36.4 KB) | 30 to 60% (24.1 to 54.6 KB) |
| 1920x1080, `-p` | 44.4 to 197.5 KB | 1% (0.4 to 1.0 KB) or none tested | 1 to 2% (0.9 to 2.0 KB) |

- The first paint of a progressive-DC file is a smooth blur of the whole frame. Judged from the screenshots, it is about as coarse as a thumbhash at 2 to 3% of the file and clearly recognisable at 10%. It sharpens in steps over roughly the first 10 to 20% of the file; the detail then arrives group by group.
- `--group_order=1` (centre first) changed nothing before the detail groups.
- For comparison in the same harness, 640x960: a lossy WebP (37 KB) had painted nothing at 15% and then filled in from the top, with 36% of the rows painted at half the bytes. A progressive JPEG (65 KB) started filling from the top at 1% and covered the whole frame, coarsely, at 15%.
- Elsewhere: with the `jxl-oxide` decoder and a 1598x1753 photo, "the 6 kB mark gives you a decent impression of the image" ([Jake Archibald](https://jakearchibald.com/2025/present-and-future-of-progressive-image-rendering/)).

**Cost of progressive DC**, mean over the ten test images at effort 4: `--progressive_dc=1` added 4.6% to the bytes at 640x960 and 4.3% at 1920x1080; `-p` added 8.8% and 8.4%. Encode time on one thread went from 18.5 ms to 21.5 ms (`--progressive_dc=1`) or 31 ms (`-p`) at 640x960, and from 44 ms to 55 ms or 73 ms at 1920x1080.

### Against a thumbhash

A ThumbHash is "a few bytes" (the demo hash is 21), carries the aspect ratio, and travels inside the API response ([thumbhash](https://evanw.github.io/thumbhash/)). The client can draw it before it asks for the image.

Progressive JXL does the same job, and does it better, when all of these hold:

- The client is Chrome 155+ or Firefox 158+.
- The file was encoded with progressive DC.
- The response has started, and the rest of the bytes take long enough to notice: a variant already in the Server's cache, on a slow link. Then the first kilobyte or two paint a blur drawn from the real image, at higher resolution than a 21-byte hash, and it refines without a visible swap.

It does not do the job in these cases:

- **Before the first byte.** A variant that is not cached has to be read, decoded, resized and encoded before the Server can send anything. Nothing can be painted from bytes that do not exist yet. For an on-request resizer this is the slow case that matters, and the hash is already in the client.
- **Safari**, on iOS and macOS: 15% of caniuse's usage. It decodes JXL and paints only the finished image.
- **Every client on the fallback**: Edge, Samsung Internet, TV runtimes, native apps. WebP has no whole-frame preview at all.
- **Images not requested yet**: rows the client has not reached, `loading="lazy"`, requests queued behind others.
- **Requests that fail or hang.** The hash stays; a progressive image never starts.

And in one case neither matters: a cached variant on a fast local network. A 50 KB poster arrives in a few milliseconds.

**Conclusion (mine).** Progressive JXL is a real improvement for two browsers on slow links. It is not a replacement for a hash, because it cannot cover the wait that an on-request resizer itself creates, and it leaves out Safari and every fallback client. `thumbhash` is one crate with no dependencies; dropping it buys nothing.

## 3. Other clients

- **Apple native (ImageIO).** `UTType.jpegxl` ("A JPEG-XL encoded image", `public.jpeg-xl`) is declared from iOS 18.2, macOS 15.2 and tvOS 18.2 ([Apple](https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct/jpegxl)). WebKit's JXL support dates from iOS 17 and macOS Sonoma ([webkit.org](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/)). That ImageIO has decoded JXL for native apps since iOS 17, tvOS 17 and macOS 14 is widely repeated and was not found in an Apple document.
- **Android platform.** No. Android's table of supported image formats lists BMP, GIF, JPEG, PNG, WebP, HEIF and AVIF, and no JPEG XL ([developer.android.com](https://developer.android.com/media/platform/supported-formats)). AOSP has no `platform/external/libjxl` (the URL returns 404), and its Skia build file does not mention JXL ([Android.bp](https://android.googlesource.com/platform/external/skia/+/refs/heads/main/Android.bp)).
- **Android image libraries.** Coil decodes through pluggable `Decoder`s and documents them as the way "to add support for custom file formats" ([Coil](https://github.com/coil-kt/coil/blob/main/docs/image_pipeline.md)). JXL comes from a third-party library that bundles its own codec: `jxl-coder`, "Supports version from Android 5.0", with separate Coil and Glide plug-ins ([jxl-coder](https://github.com/awxkee/jxl-coder)). An Android app has to ship that to show JXL.
- **Tizen.** Samsung TVs: 2026 is Tizen 10.0 on Chromium M130, 2025 is Tizen 9.0 on M120 ([Samsung](https://developer.samsung.com/smarttv/develop/specifications/web-engine-specifications.html)). Both are far below 155.
- **webOS.** LG TVs: webOS TV 26 is Chromium 132, webOS TV 25 is Chromium 120 ([LG](https://webostv.developer.lge.com/develop/specifications/web-api-and-web-engine)). Same.
- **Kodi.** Kodi's own ffmpeg build file has no `--enable-libjxl` ([xbmc](https://github.com/xbmc/xbmc/blob/master/tools/depends/target/ffmpeg/CMakeLists.txt)). Whether a Kodi built against a distribution's ffmpeg decodes JXL was not checked.
- **Embedded Chromium.** Seen here: an Electron shell reporting Chrome/152 failed to decode a JXL test image. Desktop apps follow their bundled Chromium, not the installed Chrome.

## 4. Encoding JPEG XL from Rust

### Candidates

| Crate | Does | Progressive output | Latest; releases in 12 mo | Maintainers | Licence and `deny.toml` | Native dependencies | `unsafe` in the Server | New crates |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [`jpegxl-rs`](https://crates.io/crates/jpegxl-rs) | Encode and decode, lossy and lossless, lossless JPEG recompression and reconstruction; safe wrapper over libjxl | Yes: `set_frame_option` takes any libjxl frame setting | 0.16.0+libjxl-0.12.0, 2026-10-03; 6 | One owner (`inflation`) | **GPL-3.0-or-later: fails** (`jpegxl-rs` and `jpegxl-sys`) | **libjxl (C++)**. System libjxl >= 0.12 by pkg-config, or `vendored`: cmake build of bundled libjxl, highway, brotli, skcms | No | 12 with `vendored` and no `image` |
| [`jpegxl-src`](https://crates.io/crates/jpegxl-src) | The bundled libjxl source and its cmake build | | 0.12.0, 2026-07-03; 3 | Same | BSD-3-Clause | cmake, C++ | | with the above |
| [`jxl-sys`](https://crates.io/crates/jxl-sys) | Raw bindgen bindings to a bundled libjxl 0.12.0 | Whatever libjxl offers | 0.1.12+libjxl-0.12.0, 2026-08-07; 6 | Two owners (Zetier) | MIT OR Apache-2.0: passes | **libjxl (C++)**: cmake, C++, and libclang for bindgen at build time | **Yes**, every call | 1 |
| [`jixel`](https://crates.io/crates/jixel) | Encode only, pure Rust: lossy VarDCT, lossless, lossless JPEG transcode | Yes: `progressive_dc`, `progressive_passes`, `group_order` | 0.5.7, 2026-11-06; 48 (first release 2026-05-27) | One owner (`awxkee`) | BSD-3-Clause OR Apache-2.0: **passes** | None | No | 3; 1 without default features |
| [`jxl-encoder`](https://crates.io/crates/jxl-encoder) | Encode only, pure Rust: lossy VarDCT, lossless, animation; JPEG recompression behind a feature | Yes: `with_lf_frame` ("matching libjxl's `progressive_dc >= 1`") and `ProgressiveMode` | 0.3.4, 2026-10-25; 10 (first release 2026-02-15) | Imazen (`lilith` and a team) | AGPL-3.0-only OR commercial: **passes** | None | No | 12; 9 with only `std` |
| [`zune-jpegxl`](https://crates.io/crates/zune-jpegxl) | Encode only, **lossless only** | No | 0.5.2, 2026-01-24; 2 | One owner (`etemesi254`) | MIT OR Apache-2.0 OR Zlib: passes | None | No | 2 |
| [`jxl`](https://crates.io/crates/jxl) (`jxl-rs`) | **Decode only**; the decoder in Chrome and Firefox | Decodes progressively | 0.8.0, 2026-10-19; 21 | libjxl project (`veluca93`, `tirr-c`) | BSD-3-Clause: passes | None | No | 6; 11 through [`jxl-image-rs-integration`](https://crates.io/crates/jxl-image-rs-integration) |
| [`jxl-oxide`](https://crates.io/crates/jxl-oxide) | **Decode only**, plus JPEG reconstruction | Decodes partial files | 0.12.6, 2026-05-29; 1 | One owner (`tirr-c`) | MIT OR Apache-2.0: passes | None | No | 16; 20 with its `image` feature |

Also seen and set aside: [`zenjxl`](https://crates.io/crates/zenjxl) and [`zenjxl-decoder`](https://crates.io/crates/zenjxl-decoder) fail on advisories (`paste` and `proc-macro-error2`, both unmaintained); [`gamut-jxl`](https://crates.io/crates/gamut-jxl) fails on `proc-macro-error2`, and its encoder is a static libjxl build; [`jflib-img-jxl-enc`](https://crates.io/crates/jflib-img-jxl-enc) fails on licences (`IJG`, `Unlicense` in a dependency); [`kettle-jxl`](https://crates.io/crates/kettle-jxl) (0.1.0, published today) binds the system libjxl for lossless and JPEG recompression only and calls itself "an early prototype"; [`jpxl`](https://crates.io/crates/jpxl) passes but has one release and 52 downloads.

### Notes on the four that could encode a lossy poster

- **`jpegxl-rs`** is the long-standing wrapper: on crates.io since 2020, 41 versions, and it follows libjxl closely (libjxl 0.12.0 came out 2026-07-01, the wrapper for it on 2026-07-03).
  - Its licence is the wrapper author's choice. libjxl itself is "available under a 3-clause BSD license" ([libjxl README](https://github.com/libjxl/libjxl/blob/main/README.md)), and `jpegxl-src` is BSD-3-Clause. The Server is `AGPL-3.0-only`; GPLv3 section 13 gives "permission to link or combine any covered work with a work licensed under version 3 of the GNU Affero General Public License" ([GPL-3.0](https://www.gnu.org/licenses/gpl-3.0.html#section13), read from the crate's [`COPYING`](https://docs.rs/crate/jpegxl-rs/0.16.0+libjxl-0.12.0/source/COPYING)). So the combination is allowed, but `deny.toml` needs an exception for each of the two crates.
  - The vendored build links libjxl, highway and brotli statically and the C++ standard library dynamically (`stdc++` on Linux, `c++` on Apple). On Windows it asks cmake for the `ClangCL` toolset ([jpegxl-src](https://docs.rs/crate/jpegxl-src/0.12.0/source/src/lib.rs)). Its `cfg!(windows)` and `cfg!(target_os = "linux")` checks read the build host, not the target, so each of the five targets is safest built on its own OS.
  - Without `vendored` it needs a system libjxl at least as new as the one in its version string, 0.12.0 ([build.rs](https://docs.rs/crate/jpegxl-sys/0.13.0+libjxl-0.12.0/source/build.rs)). Debian trixie has 0.11.2, so the Docker image could not use the system library with the current wrapper.
  - The bundled libjxl is 0.12.0. libjxl 0.13.0, released today, has a Security section (section 10).
- **`jxl-sys`** has a permissive licence and nothing else in its favour here: raw FFI, so the Server would hold a module of reviewed `#[expect(unsafe_code)]`; `bindgen` needs libclang on every build machine; its `lto` feature is Linux-only; its repository has no stars ([repository](https://github.com/zetier/jxl-sys)).
- **`jixel`** describes itself as "A tiny JPEG XL encoder in Rust, ported and reworked from libjxl/libjxl-tiny" ([README](https://github.com/awxkee/jixel)). The published 0.5.7 is not tiny: 320 source files, about 194,000 lines, 2,051 occurrences of `unsafe` (SIMD), seven speed levels, lossless, JPEG transcoding, gain maps. Its `EncodeConfig::progressive_dc` is documented as coding the 1:8 image "as its own squeezed modular frame ahead of the main frame, so a decoder can show previews refining from 1:256 to 1:8 before any AC data arrives" ([source](https://docs.rs/crate/jixel/0.5.7/source/src/encode_image.rs)). The API is safe functions over byte slices. The author also maintains `pic-scale`, which the earlier survey listed. Against it: five and a half months old, one person, a release every few days, no published quality or conformance figures in its README, 43k downloads.
- **`jxl-encoder`** says its output "is checked against three independent decoders (jxl-rs, jxl-oxide and libjxl's djxl)" and that it is "`#![forbid(unsafe_code)]` unless you enable the opt-in `unsafe-performance` feature". Its own comparison against cjxl 0.12 at effort 7, on 41 photos: "total size and mean butteraugli are equal"; from distance 1 to 5 its files are "1–3.5% larger" ([README](https://crates.io/crates/jxl-encoder)). That is the vendor's claim. It has efforts 1 to 10. Multi-threading is an opt-in feature. `AGPL-3.0-only` is on this repo's allow-list, so cargo-deny passes it, though the comment on that entry says it is there "for this workspace's own crates". Against it: ten months old, 46k downloads, one company.

### Cross-compiling

- The pure-Rust encoders need nothing beyond `rustc` on any of the five targets. `jixel` selects AVX, SSE and NEON paths by Cargo feature.
- libjxl through either wrapper needs cmake and a C++ compiler for each target, and on `x86_64-pc-windows-msvc` either the ClangCL toolset (`jpegxl-src`) or MSVC with Ninja if present (`jxl-sys` [build.rs](https://docs.rs/crate/jxl-sys/0.1.12+libjxl-0.12.0/source/build.rs)). The Linux binary would depend on `libstdc++.so.6` at run time. None of this was built here.

### Decoding, for JXL uploads

Both decoders pass `deny.toml`, are pure Rust and plug into `image`:

- `jxl-image-rs-integration` registers a decoding hook, after which `image::open` and format guessing handle JXL; it honours `image::Limits` for dimensions and its own buffers and returns the ICC profile ([source](https://docs.rs/crate/jxl-image-rs-integration/0.8.0/source/src/lib.rs)). The core crate also has a `sample_limit` option ([docs](https://docs.rs/jxl/0.8.0/jxl/api/struct.JxlDecoderOptions.html)). It is the decoder two browsers ship, at 23 releases in fourteen months.
- `jxl-oxide`'s `image` feature gives an `ImageDecoder` and warns that "memory limits are not strict" ([source](https://docs.rs/crate/jxl-oxide/0.12.6/source/src/integration/image.rs)). It can also rebuild the original JPEG from a recompressed file (`reconstruct_jpeg`). It has one advisory on record, fixed in the current release (section 10), and one release in the last twelve months.

**Assessment (mine).** If JXL uploads are to be accepted, take `jxl` with `jxl-image-rs-integration`. For encoding there are three honest options, each with a named price:

1. A pure-Rust encoder, `jixel` first (permissive, no dependencies, has progressive DC) and `jxl-encoder` second. Price: betting the main output format on a crate less than a year old. Section 11 is the prototype that measured their quality, speed, memory and browser support.
2. libjxl through `jpegxl-rs` with `vendored`. Price: two licence exceptions in `deny.toml`, and a C++ build on five targets in release CI.
3. No JXL encoding yet. Price: none; the `Accept` negotiation and cache key are built so that JXL can be added later.

`jxl-sys` and the ffmpeg route are worse than all three.

## 5. The ffmpeg route

- **jellyfin-ffmpeg8 has no libjxl.** Its `debian/rules` enables `--enable-libwebp`, `--enable-libsvtav1`, `--enable-libdav1d`, `--enable-libzimg` and others, and has no `--enable-libjxl` ([debian/rules](https://github.com/jellyfin/jellyfin-ffmpeg/blob/jellyfin/debian/rules)). The portable builder has a script for libwebp and none for libjxl ([scripts.d](https://github.com/jellyfin/jellyfin-ffmpeg/tree/jellyfin/builder/scripts.d)). The earlier survey was right.
- **Mainline ffmpeg encodes stills** through `libjxl` (and animations through `libjxl_anim`). Its options are `effort` (1 to 9, default 7), `distance`, `modular` and `xyb`, and the only frame settings it passes to libjxl are effort and modular ([libjxlenc.c](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/libjxlenc.c)). **It cannot write a progressive-DC file.**
- **Measured** with this machine's ffmpeg n9.0.2: `-c:v libjxl -effort 4 -distance 1.9` gave files within three bytes of `cjxl -q 80 -e 4`, and Firefox painted them exactly like default files (section 2). Whole-process times, PNG decode and process start included, mean of ten images:

| Output | 640x960 | 1920x1080 |
| --- | --- | --- |
| `-c:v libjxl -effort 3` | 55 ms | 97 ms |
| `-c:v libjxl -effort 4` | 57 ms | 99 ms |
| `-c:v libjxl -effort 7` | 85 ms | 158 ms |
| `-c:v libwebp -quality 80` | 72 ms | 156 ms |

So an ffmpeg that has libjxl is fast enough, but it is not the ffmpeg the Docker image ships, it gives up the early preview, and ADR 0026 lets the Server run with no ffmpeg at all.

## 6. Speed on request

libjxl publishes what each effort level does, not how long it takes: efforts 1 and 2 use "only 8x8 blocks", 3 and 4 add entropy-coding refinements, 5 adds variable block sizes, adaptive quantisation and chroma from luma, 6 and 7 add more searches, 8 and above add Butteraugli iterations ([encode_effort.md](https://github.com/libjxl/libjxl/blob/main/doc/encode_effort.md)). imgproxy's default JXL effort is 4 ([imgproxy](https://docs.imgproxy.net/configuration/options)).

Measured here. Encode time is the encoder alone: for libjxl, the megapixels per second that `cjxl` reports for its own encode, turned into milliseconds; for libwebp and libjpeg-turbo, an in-process Pillow `save` to memory. Best of three runs per image, mean of ten images, input already resized. `cjxl -q 80` (distance 1.9); WebP at libwebp's default method 4.

| Encoder and setting | 640x960, one thread | 640x960, 32 threads | 1920x1080, one thread | 1920x1080, 32 threads |
| --- | --- | --- | --- | --- |
| libjxl effort 1 | 15 ms | 12 ms | 37 ms | 22 ms |
| libjxl effort 2 | 16 ms | 11 ms | 37 ms | 22 ms |
| libjxl effort 3 | 17 ms | 12 ms | 40 ms | 23 ms |
| libjxl effort 4 | 19 ms | 14 ms | 44 ms | 28 ms |
| libjxl effort 5 | 78 ms | 24 ms | 225 ms | 46 ms |
| libjxl effort 6 | 133 ms | 35 ms | 396 ms | 75 ms |
| libjxl effort 7 (default) | 142 ms | 37 ms | 442 ms | 86 ms |
| libjxl effort 4, `--progressive_dc=1` | 22 ms | | 55 ms | |
| libjxl effort 4, `-p` | 31 ms | | 73 ms | |
| libwebp quality 80 | 29 ms | | 92 ms | |
| libwebp quality 90 | 34 ms | | 106 ms | |
| libjpeg-turbo quality 80, progressive | 5 ms | | 16 ms | |

- Efforts 1 to 4 are faster than libwebp. There is a cliff between 4 and 5, where the block-size search begins: four to five times slower on one thread.
- Mean file size barely moves with effort: 48.6 KiB at effort 3, 50.0 at 4, 48.3 at 5 and 49.2 at 7 for 640x960. Higher effort buys more consistent quality, not smaller files; the same document says so.
- The whole process (`cjxl` start, read, encode, write) took 21 to 22 ms at effort 3 to 4 for 640x960 and 45 to 49 ms for 1920x1080.
- On request, the sensible range is effort 3 or 4 on one thread per image. Effort 7 on one thread is close to half a second for a backdrop.

This is libjxl. Section 11 times `jixel` and `jxl-encoder`.

## 7. Size at web sizes

**What others measured.**

- **The JPEG XL authors** (Cloudinary and Google Research; first-party), summarising the CID22 subjective data set of 250 images: for portraits, "JPEG XL performs better than AVIF, which performs better than WebP, which performs better than JPEG, with a gap of roughly 10-15% between each"; for city and outdoor scenes, "JPEG XL and AVIF have a similar performance, both outperforming WebP and JPEG by 20-30%"; for illustrations, "diagrams, charts, logos, text: here AVIF outperforms JPEG XL and WebP, which perform similarly, by about 20-30%". They add that "the amount of experimental data available is still limited" ([arXiv 2506.05987](https://arxiv.org/abs/2506.05987), section 10.2.1).
- **Cloudinary** (vendor; the author is a JPEG XL co-creator), on images of "about 1 megapixel each" scored with SSIMULACRA2: at medium web quality JPEG XL "can reach a similar compression density [to AVIF] but encodes significantly faster"; at visually lossless quality libjxl is 20% smaller than libavif; WebP cannot reach that quality at all because its 4:2:0 subsampling is mandatory ([Cloudinary, 2024-02-28](https://cloudinary.com/blog/jpeg-xl-and-the-pareto-front)).
- **The AVIF team at Google** (first-party to AVIF and WebP), by MS-SSIM BD-rate from 0.1 to 3 bits per pixel: AVIF best, then JPEG XL, then WebP, then JPEG; no percentages in the text ([avif-comparison, 2022-12-14](https://storage.googleapis.com/avif-comparison/index.html)). This was the data behind Chrome's 2022 removal.
- **Chrome in 2026** claims "30-50% better compression than JPEG" and expects JPEG XL to matter most "for high-fidelity or lossless compression, especially for photographs" ([developer.chrome.com](https://developer.chrome.com/blog/jpeg-xl-in-chrome)).
- **Next.js** on its two formats: "AVIF generally takes 50% longer to encode but it compresses 20% smaller compared to WebP" ([docs](https://nextjs.org/docs/app/api-reference/components/image)).

No independent study at 300 to 800 px was found. The published comparisons use images of a megapixel or more.

**What the local test showed.** Ten illustration-like images, no perceptual metric from the JPEG XL family available (no `ssimulacra2` or `butteraugli` tool is installed), so three other metrics through ffmpeg: SSIM on RGB, VMAF, and XPSNR on luma. Mean over the ten 640x960 files:

| Setting | Size | SSIM | VMAF | XPSNR-Y |
| --- | --- | --- | --- | --- |
| JXL `-q 80`, effort 4 | 50.0 KiB | 0.903 | 94.6 | 45.5 dB |
| JXL `-q 80`, effort 7 | 49.2 KiB | 0.912 | 94.5 | 46.9 dB |
| WebP quality 75 | 32.6 KiB | 0.922 | 90.8 | 46.0 dB |
| WebP quality 80 | 40.3 KiB | 0.929 | 92.3 | 47.3 dB |
| WebP quality 85 | 50.9 KiB | 0.936 | 93.5 | 48.6 dB |
| WebP quality 90 | 68.7 KiB | 0.944 | 94.7 | 50.5 dB |
| JPEG quality 80, progressive | 65.0 KiB | 0.924 | 94.2 | 47.3 dB |

- The same number does not mean the same quality: JXL at 80 is 24% larger than WebP at 80.
- Matched by VMAF, JXL at 50 KiB equals WebP quality 90 at 69 KiB: JXL 27% smaller (30% at 1920x1080). Matched by SSIM or XPSNR, WebP at 33 KiB already scores above JXL at 50 KiB.
- The metrics disagree, the content is the kind where the authors themselves rate JXL level with WebP, and none of the three metrics is one JXL is tuned for. **This test does not settle size.** A decision needs real posters and backdrops, judged by eye or by SSIMULACRA2.

## 8. Lossless JPEG recompression

- **What it is.** libjxl can repack a JPEG's own DCT coefficients into a JXL file and later rebuild the original file byte for byte. `cjxl` does this by default for JPEG input ([libjxl README](https://github.com/libjxl/libjxl/blob/main/README.md)). The data needed for exact reconstruction sits in a separate `jbrd` box, which "is not needed to display a recompressed JPEG image" ([format overview](https://github.com/libjxl/libjxl/blob/main/doc/format_overview.md)).
- **What it saves.** "By about 20%" ([arXiv 2506.05987](https://arxiv.org/abs/2506.05987)); "an average of 20%" ([webkit.org](https://webkit.org/blog/14445/webkit-features-in-safari-17-0/)); "On average, an encoded image is 22% smaller" ([chromestatus](https://chromestatus.com/feature/5678152091172864)). Measured here on the eleven JPEG wallpapers this machine has (seven are variants of one design): 11.2% to 22.9% per file, 19.6% in total, at effort 7; effort 3 gave the same sizes. `djxl` rebuilt every original bit for bit. `cjxl` took about 50 ms for a 2-megapixel file and 0.8 to 1.0 s for 8 to 11 megapixels, one thread, whole process.
- **Do browsers show such files?** Yes, they are ordinary JXL images. Checked in Firefox 160 with one recompressed 1920x1080 file: it rendered, differing from Firefox's rendering of the original JPEG by a mean of 0.37 levels out of 255. It did not paint early: nothing at 20% of the bytes.
- **From Rust.** `jpegxl-rs`: `JxlEncoder::encode_jpeg` and a decoder mode that returns the reconstructed JPEG ([encode.rs](https://docs.rs/crate/jpegxl-rs/0.16.0+libjxl-0.12.0/source/src/encode.rs), [decode.rs](https://docs.rs/crate/jpegxl-rs/0.16.0+libjxl-0.12.0/source/src/decode.rs)). `jixel`: `encode_jpeg_lossless`, "keeping the ability to reconstruct the original bytes" ([source](https://docs.rs/crate/jixel/0.5.7/source/src/jpeg/mod.rs)). `jxl-encoder`: its `jpeg-reencoding` feature, which raises its new-crate count from 12 to 31. Reconstruction in pure Rust: `jxl-oxide`'s `reconstruct_jpeg` ([source](https://docs.rs/crate/jxl-oxide/0.12.6/source/src/lib.rs)). The `jxl` crate exposes the `jbrd` box but no reconstruction function was found in it.
- **Relevance.** It applies to stored originals, not to resized variants: a resize re-encodes the pixels. Keeping originals as recompressed JXL would save about a fifth of their disk space and keep the exact JPEG recoverable, at the price of a JXL encoder and decoder in the storage path and a second or so of CPU per large original. For a library whose originals total a few gigabytes that is a small saving for a real dependency.

## 9. Precedent

- **Next.js image optimizer.** No. `type ImageFormat = 'image/avif' | 'image/webp'`, default `['image/webp']` ([image-config.ts](https://github.com/vercel/next.js/blob/canary/packages/next/src/shared/lib/image-config.ts)). A JXL source is passed through untouched: it is in `BYPASS_TYPES` ([transform.ts](https://github.com/vercel/next.js/blob/canary/packages/next/src/server/image-optimizer/transform.ts)).
- **Jellyfin.** No. Its `ImageFormat` enum is Bmp, Gif, Jpg, Png, Webp, Svg ([ImageFormat.cs](https://github.com/jellyfin/jellyfin/blob/master/MediaBrowser.Model/Drawing/ImageFormat.cs)); it negotiates WebP on `Accept` and sends `Vary: Accept` ([ImageController.cs](https://github.com/jellyfin/jellyfin/blob/master/Jellyfin.Api/Controllers/ImageController.cs)).
- **Plex.** Not verified for JXL. `docs/research/jellyfin-and-plex-images.md` records that Plex takes an explicit `format` of jpg, png or ppm and that staff call WebP output unsupported.
- **Immich.** Partly. Its `ImageFormat` setting is `jpeg`, `jxl` or `webp` on the main branch ([enum.ts](https://github.com/immich-app/immich/blob/main/server/src/enum.ts)); the admin picks one for all thumbnails (the documented default for thumbnails is `webp`, [config](https://github.com/immich-app/immich/blob/main/docs/docs/install/config-file.md)), it is not negotiated per request, and video thumbnails still fall back to WebP ("TODO: write JPEG XL for video thumbnails", [media.service.ts](https://github.com/immich-app/immich/blob/main/server/src/services/media.service.ts)). Which release carries it was not checked.
- **imgproxy.** Yes, opt-in. `IMGPROXY_AUTO_JXL` "enables JPEG XL support detection", `IMGPROXY_ENFORCE_JXL` forces it, the order when several are on is "JPEG XL, AVIF, WebP", the default effort is 4, and it cannot write animated JXL ([options](https://docs.imgproxy.net/configuration/options), [formats](https://docs.imgproxy.net/image_formats_support)).
- **Fastly Image Optimizer.** Yes. `format=jxl` and `format=pjxl` ("Progressive JPEGXL"); `auto` ranks JPEG XL first, then AVIF, then WebP; JPEG XL and AVIF need its Professional tier ([Fastly](https://www.fastly.com/documentation/reference/io/format/)).
- **Cloudinary.** Yes on some plans. `f_auto` can deliver "AVIF, JPEG XL, or WebP"; on the bandwidth billing metric "AVIF, animated AVIF and JPEG XL aren't supported for `f_auto` by default" ([Cloudinary](https://cloudinary.com/documentation/image_optimization)).
- **Cloudflare Images.** No. `format` takes `auto`, `avif`, `webp`, `jpeg`, `baseline-jpeg`, `json` ([Cloudflare](https://developers.cloudflare.com/images/transform-images/transform-via-url/)).
- **Akamai Image and Video Manager.** No. Its format table has a JPEG XL row with neither input nor output ([Akamai](https://techdocs.akamai.com/ivm/docs/supported-image-and-video-formats)).
- **imgix.** Input only ([imgix](https://docs.imgix.com/references/supported-file-formats)).

Of six image services, three can negotiate JXL (imgproxy, Fastly, Cloudinary), each as an option or a paid tier. No media server negotiates it.

## 10. Risks

**libjxl advisories.**

- libjxl 0.13.0 was released on 2026-11-11 with a Security section: use of uninitialised memory in the low-memory render pipeline, "potential use-after-free writes across decoder resets" in JPEG reconstruction, integer overflows, out-of-bounds guards, and a brotli update for a CVE ([changelog](https://github.com/libjxl/libjxl/blob/main/CHANGELOG.md)). No Rust wrapper bundles 0.13.0 yet; `jpegxl-src` and `jxl-sys` bundle 0.12.0.
- Earlier: 0.12.0 (2026-07-01) listed about thirty hardening fixes "across the core decoder/encoder"; 0.11.2 (2026-02-10) fixed CVE-2025-12474 and CVE-2026-1837; 0.11.1 fixed CVE-2024-11403 and CVE-2024-11498; older releases fixed CVE-2023-35790, CVE-2023-0645, CVE-2021-22563 and CVE-2021-22564 (same changelog; the 2021 pair is also in [GitHub advisories](https://github.com/libjxl/libjxl/security/advisories)).
- Most of these are decoder bugs. An encode-only use of libjxl, fed pixels the Server made itself, is outside most of them, the same argument the earlier survey made for libwebp. The 0.13.0 notes do include encoder-side fixes.
- Browsers moved away from this code: Chrome's condition for shipping was "a performant and memory-safe JPEG XL decoder" (below), and both Chrome and Firefox ship `jxl-rs`. Safari uses the C++ decoder, per chromestatus ([chromestatus](https://chromestatus.com/feature/5114042131808256)).

**Rust crates.** OSV has no advisory for `jpegxl-rs`, `jpegxl-sys`, `jpegxl-src`, `jxl-sys`, `jxl`, `jixel`, `jxl-encoder` or `zune-jpegxl`. `jxl-oxide` has one, "`FrameBuffer::new` creates out-of-bounds slices on overflow", fixed in 0.12.6 ([GHSA-66m8-c62j-h6v5](https://github.com/advisories/GHSA-66m8-c62j-h6v5)). An empty record for a crate under a year old says little.

**Bitstream and standard.** ISO/IEC 18181-1 (core coding system) is published in its second edition, 2024; part 2 (file format) is in its third edition, 2026, with a fourth in committee draft; part 3 (conformance) is at its second edition, 2025; part 4 (reference software) dates from 2022 ([jpeg.org](https://jpeg.org/jpegxl/workplan.html)). No statement that the bitstream is frozen was found on the pages read. libjxl 0.12 added out-of-order `jxlp` boxes, an opt-in output mode that "requires a decoder that supports ftyp minor version 1"; the default output is unchanged ([changelog](https://github.com/libjxl/libjxl/blob/main/CHANGELOG.md)). An encoder should stay on defaults that older decoders, Safari's included, can read.

**Chrome's history.**

- 2021-03-30: Intent to Prototype; a libjxl-based decoder sat behind a flag from Chrome 91 ([blink-dev](https://groups.google.com/a/chromium.org/g/blink-dev/c/WjCKcBw219k), [caniuse](https://caniuse.com/jpegxl)).
- 2022-10-31: a note on the Chromium bug announced removal in Chrome 110, giving among its reasons that "there is not enough interest from the entire ecosystem" and that the format "does not bring sufficient incremental benefits over existing formats" (as quoted by [The Register](https://www.theregister.com/2022/10/31/jpeg_xl_axed_chrome/), a secondary source; the bug page could not be read). On 2022-11-11 Jim Bankoski wrote on blink-dev: "we've decided to stop Chrome's JPEG XL experiment and remove the code associated with the experiment" (same thread).
- 2025-11-21: Rick Byers, for the Chrome architecture leads: "Since JPEG XL was last evaluated, Safari has shipped support and Firefox has updated their position... we would welcome contributions to integrate a performant and memory-safe JPEG XL decoder in Chromium. In order to enable it by default in Chromium we would need a commitment to long-term maintenance" (same thread).
- Chrome 145 (early 2026): the `jxl-rs` decoder behind a flag ([caniuse](https://caniuse.com/jpegxl); the feature is present and off by default in the [145 tag](https://chromium.googlesource.com/chromium/src/+/refs/tags/145.0.7632.162/third_party/blink/common/features.cc)). 2026-08-24: Intent to Ship. Chrome 155, 2026-10-06: on by default (section 1).

**What could still go wrong.**

- The feature sits behind a Finch name, `JXLImageFormat`, so Google can switch it off remotely; it has been on by default for five weeks. With `Accept` negotiation that failure is harmless: a browser that stops decoding stops asking.
- Edge may stay out, as it did with AVIF for years. That is 5% of global usage on the fallback for good.
- Encoder churn. Three of the four usable encoders are single-maintainer, and two are months old.
- Cache size. A second format doubles the stored variants for every size that both kinds of client ask for.

## 11. Encoders measured

Measured on 2026-11-12 for #728, which picks the encoder #739 uses. Both pure-Rust candidates were compiled and run against the six gates #728 sets; libjxl was not a candidate (section 4: two GPL exceptions and a C++ build on five targets).

### Result

| Gate | `jxl-encoder` 0.3.4, effort 3, distance 1.5 | `jixel` 0.5.7, `UltraFast`, distance 1.2, lossless alpha |
| --- | --- | --- |
| 1. Chrome, Firefox and Safari show the files | Chrome 157 and Firefox 160 showed all six, each within 0.33 of 255 (mean) of `djxl`'s decode. Safari: not yet checked, by hand on a Mac (#728) | The same |
| 2. Chrome and Firefox paint a blurred full frame early | Opaque files: from 1 to 2% of the file (0.6 to 1.1 KB), in both. Files with alpha: no, and no encoder's do (below) | Opaque files: from 1 to 2% (0.4 to 0.9 KB). Files with alpha: no |
| 3. One job within 128 MiB above idle | Worst 110.7 MiB, the RGBA PNG to a Backdrop | Worst 86.7 MiB, the RGBA PNG to a Backdrop, in the resize |
| 4. Encodes on the calling thread | Yes. Without its `parallel` feature it has no threads | Yes, with `with_num_threads(1)`. Its default is one thread per core |
| 5. 1920x1080 in 200 ms or less on one thread | 71 to 83 ms | 63 to 71 ms |
| 6. `cargo deny`; `cargo check` on the five targets | Pass; pass | Pass; pass |
| Peak memory of its own per output pixel, 2 MP and up | 49 to 51 bytes | 14 to 16 bytes |
| Size against the Server's JPEG at the same look | 40 to 52% smaller | 46 to 58% smaller; 8 to 18% smaller than `jxl-encoder` |
| New crates in `Cargo.lock` | 9 | 1 |

Both pass every gate run here; Safari is still to be checked. By #728's ranking (memory per output pixel, then speed, then size, then upkeep) **`jixel` wins on the first criterion**, and on the next two. `jxl-encoder` is better kept: a company, output checked against three decoders, `#![forbid(unsafe_code)]`; `jixel` has one maintainer, 48 releases since 2026-05-27 and 2,051 uses of `unsafe` (section 4). Upkeep comes last, so it was not reached.

**Pixel limit.** The encode stage, the encoder plus the image it encodes, needs about 17.0 MiB per megapixel plus 5.4 MiB with `jixel` for RGBA, and 15.6 MiB plus 5.5 MiB for RGB. 128 MiB holds 7.2 megapixels of RGBA and 7.8 of RGB, so the limit is **7 megapixels**. On the Server's width list, 2:3 images fit up to 1920 px (5.5 MP) and 16:9 up to 2560 px (3.7 MP); a 2:3 image at 2560 px (9.8 MP) goes as JPEG. `jxl-encoder` would fit 2.4 megapixels. The limit bounds the encode only: decode and resize stay under #725's rules, and the RGBA PNG to 1920x2880 peaked at 113 MiB in the resize.

### Method

- **Machine and pipeline.** The machine of `image-memory-use.md` (Ryzen 9 7950X3D, glibc 2.44, rustc 1.99.1), load average under 1. A scratch crate outside the repo runs the Server's pipeline: `image` 0.25.10 decodes from the file; `fast_image_resize` 6 resizes with Lanczos3, cropping the centre to the output's shape, alpha premultiplied in place, resized with `use_alpha(false)` and divided back; the source is dropped; one encode follows on the calling thread.
- **Inputs.** The three images of `image-memory-use.md`: a 3840x2160 JPEG, a 2000x3000 JPEG and a 4000x4000 RGBA PNG. All three are synthetic (noise and grain), not film art. Each becomes a Poster at 640x960 and a Backdrop at 1920x1080; the scaling runs go up to 2560x3840.
- **Features.** `jxl-encoder` with `default-features = false, features = ["std"]`: no `parallel`, no `unsafe-performance`, no Butteraugli loop, which only effort 8 and up use. `jixel` with `default-features = false, features = ["avx", "sse", "neon"]`. Its defaults also turn on `wasm`, `splines` and `extraslow`, and `extraslow` pulls in two crates.
- **Memory** as in `image-memory-use.md`: `VmHWM` reset at each stage, exact `Rss` at stage edges, pinned to one CPU, transparent huge pages off, five runs, highest kept. With `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072` every figure was the same or up to 6 MiB lower; the tables give glibc's defaults.
- **Threads.** `Threads` in `/proc/self/status` before and after the encode, and the encode's CPU time for the whole process (`RUSAGE_SELF`) against the calling thread's (`RUSAGE_THREAD`). For both encoders no thread was added, and no CPU time was spent on another thread.
- **Time.** One process per case, a first encode, then 15 timed ones; best and median. The first encode in a process is 10 to 30 ms slower.
- **Quality.** SSIMULACRA2 (the `ssimulacra2` crate 0.5.1) of `djxl`'s decode against the resized source, alpha flattened on mid-grey; then 2x crops by eye.
- **Browsers.** Firefox 160.0 driven over WebDriver BiDi, and Chrome for Testing 157.0.8103.78 over the DevTools protocol, both headless. The local server of section 2 sends the first N bytes with the full `Content-Length` and then stalls, and each browser is screenshotted 1.2 s later. Prefixes of 1, 2, 3, 5, 10, 20, 30, 50 and 70%, and the whole file. "Painted" means at least 99% of the image's opaque area is covered.
- **Safari** gets a page of the same files, which reports whether each one decoded, for a hand check on a Mac.

### Choosing the distance and effort

The distance is the one at which an encoder's files look like the JPEGs the Server sends today: JPEG quality 80, from #725. Over the four opaque cases, mean SSIMULACRA2 is 81.35 for JPEG, 81.34 for `jxl-encoder` at distance 1.5, and 81.23 for `jixel` `UltraFast` at 1.2. The two encoders' distances don't mean the same thing: at 1.5, `jixel`'s opaque files are 32 to 46% smaller than `jxl-encoder`'s and score 4.6 points lower on average (76.8 against 81.3).

| Case | JPEG q80: KB, SSIMULACRA2 | `jxl-encoder`, effort 3, distance 1.5 | `jixel`, `UltraFast`, distance 1.2 |
| --- | --- | --- | --- |
| 4K JPEG to Poster | 57.3, 81.9 | 31.4, 82.7 | 28.5, 82.8 |
| 4K JPEG to Backdrop | 197.5, 82.0 | 98.8, 81.9 | 91.3, 81.8 |
| 2000x3000 JPEG to Poster | 67.3, 82.6 | 40.2, 83.4 | 36.6, 83.4 |
| 2000x3000 JPEG to Backdrop | 219.5, 78.9 | 105.6, 77.3 | 93.1, 76.9 |
| RGBA PNG to Poster | sent as stored | 58.1, 85.1 | 48.4, 84.5 |
| RGBA PNG to Backdrop | sent as stored | 125.4, 85.4 | 103.1, 84.2 |

KB is 1000 bytes. By eye at 2x, both JPEG XL encoders smooth the grain a little more than the JPEG does, and the JPEG shows faint blocks; the two encoders' crops can't be told apart.

The effort is the lowest whose files look acceptable at that distance:

- **`jxl-encoder`.** Efforts 1 to 4 decode to identical pixels at a given distance and differ only in coding. Effort 3 files are 12 to 25% smaller than effort 1's and 2's, for 5 MiB less memory and 6 ms more at 1920x1080, so effort 3. Effort 5 adds the block-size search and took 271 ms at 1920x1080; 6 took 501 ms and 7 took 568 ms.
- **`jixel`.** `UltraFast`, its lowest. `Fastest` and `Fast` look a little better at the same distance but took 137 to 149 ms and 262 ms at 1920x1080 for opaque images.
- **`jixel`'s alpha.** By default `jixel` codes alpha lossily at the frame's distance, which took 251 ms for the RGBA Backdrop at distance 1.2. With `with_alpha_distance(0.0)` alpha is lossless, as libjxl and `jxl-encoder` code it by default, and the same encode took 70 ms. Its own memory fell from 28 to 30 bytes per output pixel to 15 to 16. All `jixel` figures in this section use lossless alpha.
- **Progressive DC** is turned on with `with_lf_frame(true)` and `with_progressive_dc(true)`. `jxl-encoder`'s `with_effort` rebuilds its settings and drops `lf_frame`, so the effort has to be set first. Progressive DC cost `jxl-encoder` 4.6 to 7.1% more bytes. It cost `jixel` nothing: its files came out 0.2 to 0.6% smaller.

### Memory and time

Peak of the whole job above idle, and the encoder's own share: the encode stage's peak less the memory in use before it, which is mostly the image being encoded. Times are for the encode alone, best and median of 15.

| Case | `jxl-encoder`: job, own MiB | encode ms | `jixel`: job, own MiB | encode ms |
| --- | --- | --- | --- | --- |
| 4K JPEG to Poster | 33.5, 30.4 | 25 / 25 | 30.6, 11.8 | 21 / 21 |
| 4K JPEG to Backdrop | 106.7, 99.4 | 71 / 71 | 42.7, 30.0 | 63 / 64 |
| 2000x3000 JPEG to Poster | 33.5, 30.4 | 26 / 26 | 25.5, 11.7 | 21 / 21 |
| 2000x3000 JPEG to Backdrop | 106.5, 99.2 | 74 / 74 | 37.9, 30.6 | 68 / 69 |
| RGBA PNG to Poster | 74.2, 31.1 | 31 / 31 | 74.5, 23.3 | 26 / 26 |
| RGBA PNG to Backdrop | 110.7, 101.4 | 82 / 83 | 86.7, 31.3 | 70 / 71 |

The jobs from the RGBA PNG peak at 74 to 87 MiB in decode and resize, whichever encoder follows. Encode stage above idle by output size, from which the pixel limit above is fitted:

| Output | `jxl-encoder` RGB | `jxl-encoder` RGBA | `jixel` RGB | `jixel` RGBA |
| --- | --- | --- | --- | --- |
| 1280x720, 0.9 MP | 48.9 | 50.9 | 19.8 | 26.5 |
| 1920x1080, 2.1 MP | 106.7 | 110.3 | 37.9 | 40.7 |
| 2560x1440, 3.7 MP | 187.1 | 194.5 | 62.3 | 68.1 |
| 1920x2880, 5.5 MP | 277.0 | 290.5 | 90.7 | 99.5 |
| 2560x3840, 9.8 MP | 490.6 | | 159.0 | |

### Early paint

Share of the image's opaque area painted in Firefox 160 when the file stops at each point. Chrome 157 gave the same table for every file, as both use the `jxl` decoder.

| File | 1% | 2% | 5% | 30% | 50% | 70% |
| --- | --- | --- | --- | --- | --- | --- |
| Opaque, either encoder, progressive DC | 0 or 1 | 1 | 1 | 1, once 0 (below) | 1 | 1 |
| Opaque, either encoder, no progressive DC | 0 | 0 | 0 | 0 or 1 | 1 | 1 |
| RGBA, `jxl-encoder` with its LF frame | 0 | 0 | 0 | 0 | 0.20 to 0.22 | 0.57 to 0.68 |
| RGBA, `cjxl -e 3 --progressive_dc=1` | 0 | 0 | 0 | 0.04 to 0.08 | 0.20 to 0.32 | 0.61 to 0.68 |
| RGBA, `jixel`, lossless alpha | 0 | 0 | 0 | 0 | 0.20 to 0.28 | 0.61 to 0.68 |

- **Files with alpha get no early blur**, whichever encoder wrote them, libjxl included. They fill in by rows of groups from about half the file. `jixel` ignores progressive DC on frames with alpha (its files are byte-identical with it on or off), but that changes nothing a browser shows. Images with transparency rely on the thumbhash.
- **The blur can drop out for a moment.** For one cut between 25 and 30% of the file, both browsers showed nothing where the cuts on either side showed the blurred frame. That happened once in 97 to 98 cuts at every 1% for each of four Posters, `jxl-encoder`'s as well as `jixel`'s. With the rest of the file sent after a 3 s stall at that cut, the whole image showed. It is the decoder's behaviour, not either encoder's.
- Chrome 157 sends `image/jxl,image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8` with image requests.

### Not verified here

- Safari, until the hand check is done. Nothing in this section was run on a Mac, on Windows or on ARM; those targets were only checked with `cargo check`, so `jixel`'s NEON code has not run.
- Real posters and backdrops. The inputs are synthetic, and quality was judged by one metric and by eye.
- Times on a small device. They are for one core of a desktop CPU and will be several times longer there.
- `jxl-oxide`. `jixel`'s files were decoded by `djxl` (libjxl 0.12.0) and by the `jxl` decoder in two browsers, not by `jxl-oxide`.

## Findings that differ from a year-old memory

- **Chrome ships JPEG XL.** A year ago Chrome had removed it and had only just said it would welcome a memory-safe decoder. It has been on by default since Chrome 155.
- **Firefox ships it too**, since 158, on desktop and Android, with the same Rust decoder.
- **Both paint partial data.** A year ago no shipping browser did; Safari still does not.
- **Edge is the holdout**, and it is not a matter of Chromium version: the decoder is a build switch.
- **`cjxl -p` now produces an early preview.** Before libjxl 0.12 it did not unless `--progressive_dc` was added.
- **Pure-Rust lossy JPEG XL encoders exist** (`jixel`, `jxl-encoder`), and both pass this repo's cargo-deny. The earlier survey said one "was not looked for".
- **`jpegxl-rs` is still GPL**, and a permissive raw binding (`jxl-sys`) now exists beside it.
- **`jxl` (jxl-rs) went from a first release in 2025-09 to the decoder inside two browsers.**
- **libjxl had two releases with security sections in 2026**, the second one today.
- **Immich has a JXL thumbnail setting on its main branch; Next.js still accepts only AVIF and WebP.**

## Not verified

- Chrome's progressive painting was read from source, not seen: no Chrome is installed here. Section 11 later saw it in Chrome for Testing 157, painting exactly as Firefox 160 did.
- JXL on Chrome for Android, Android WebView and Firefox for Android was taken from chromestatus, MDN data and Mozilla's intent; no Android device was tested.
- What Edge sends in `Accept`, whether it has a flag, and why Microsoft left JXL out. Only the absence of support is established.
- Safari's behaviour was taken from WebKit source, the WebKit bug and Mozilla's statement; no Safari was run. Whether Safari on macOS Ventura and Monterey really decodes JXL, as the WebKit post says, was not checked elsewhere.
- That Chrome on iOS simply follows WebKit, and what chromestatus's "iOS 155" milestone refers to.
- Which Chromium Samsung Internet 31 is built on, and whether Opera, Brave and Vivaldi decode JXL; caniuse lists Opera 139 as supporting it.
- That ImageIO decodes JXL for native apps on iOS 17, tvOS 17 and macOS 14. Apple's documentation only shows the `UTType` constant, from 18.2 and 15.2.
- Kodi built against a system ffmpeg; Plex's handling of JXL.
- **No crate was compiled or run** for sections 1 to 10. Section 11 measures `jixel` and `jxl-encoder`. Unknown for `jpegxl-rs` and `jxl-sys`: whether they build on each of the five release targets, above all `x86_64-pc-windows-msvc` and the two macOS targets, and how cargo-dist's runners would cope with cmake.
- The libjxl timings are from `cjxl`'s own report of its encode speed. Calling libjxl from Rust in-process was not timed.
- Decode time in browsers, and decode cost on phones, were not measured. Mozilla's intent mentions a larger gap to other formats on small images.
- Size at matched perceptual quality. The local metrics disagreed, the test images are not film posters or stills, and no study at 300 to 800 px was found.
- How much of the 0.13.0 security work touches encoding, and whether any of it is reachable when libjxl only ever encodes the Server's own pixels.
- The exact text of the 2022 Chromium bug comment; it is quoted from The Register.
- The blink-dev Intent to Ship, the Mozilla thread, the Chrome blog posts and the vendor documentation were read as extracts, not in full.
- cargo-deny ran `licenses`, `advisories` and `sources` only, as in the earlier survey.
