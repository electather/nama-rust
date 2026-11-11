# Memory use of image decode, resize and encode

Measured on 2026-11-11 for the stored-images work. The question: how much resident memory does one image job take when the Server does it in-process with the crates that [rust-crates-for-stored-images.md](rust-crates-for-stored-images.md) recommends (`image` 0.25, `fast_image_resize` 6, `webp` 0.3), how much when a child ffmpeg does it, and whether the memory comes back afterwards.

Every number is from one machine (see [Method](#method)). MiB is 1024 x 1024 bytes; the kernel's "kB" is 1024 bytes. "Baseline" is the resident set size (RSS) of the same test binary doing nothing: 3.3 MiB.

## Summary

In-process rows give the job's peak RSS minus the baseline. Child rows give the child's whole peak RSS, since all of it is extra to the Server. Times are best run / median, wall clock; in-process times run from the start of decode to the end of encode, child times from spawn to exit.

| Job | Peak RSS the job adds, MiB | Time, ms |
| --- | --- | --- |
| **In-process: `image` decode, `fast_image_resize` Lanczos3, encode** | | |
| 4K JPEG (3840x2160), decode only | 26.2 | 24 / 26 |
| 4K JPEG to 640 px WebP | 29.9 | 41 / 46 |
| 4K JPEG to 640 px JPEG | 30.0 | 34 / 36 |
| 4K JPEG to 1920 px WebP | 43.2 | 118 / 128 |
| 4K JPEG, progressive, to 640 px WebP | 49.5 | 86 / 100 |
| 2000x3000 JPEG, decode only | 18.9 | 21 / 22 |
| 2000x3000 JPEG to 640 px WebP | 26.1 | 56 / 61 |
| 2000x3000 JPEG to 1920 px WebP | 51.3 | 272 / 281 |
| 2000x3000 JPEG, progressive, to 640 px WebP | 35.9 | 78 / 88 |
| 4000x4000 RGBA PNG, decode only | 61.7 | 80 / 83 |
| 4000x4000 RGBA PNG to 640 px WebP, resizer defaults | 135.2 | 150 / 155 |
| 4000x4000 RGBA PNG to 640 px WebP, alpha premultiplied in place | 74.2 | 134 / 139 |
| 4000x4000 RGBA PNG to 1920 px WebP, alpha premultiplied in place | 111.2 | 560 / 699 |
| **In-process with scaled JPEG decoding (`jpeg-decoder` 0.3.2)** | | |
| 4K JPEG to 640 px WebP, decoded at 1/4 size | 4.9 | 32 / 34 |
| 2000x3000 JPEG to 640 px WebP, decoded at 1/2 size | 10.4 | 43 / 45 |
| 4K JPEG, progressive, to 640 px WebP, decoded at 1/4 size | 26.4 | 81 / 103 |
| **Child process: system ffmpeg n9.0.2, one thread (default threads in brackets)** | | |
| `ffmpeg -version`, the cost of starting it | 43.8 | 25 / 26 |
| 4K JPEG to 640 px WebP | 63.4 (68.3) | 61 / 62 (71 / 72) |
| 4K JPEG to 640 px JPEG | 64.1 (72.7) | 50 / 52 (63 / 65) |
| 4K JPEG to 1920 px WebP | 65.6 (72.4) | 129 / 132 (140 / 145) |
| 4K JPEG, progressive, to 640 px WebP | 86.9 (91.9) | 77 / 81 (86 / 90) |
| 2000x3000 JPEG to 640 px WebP | 60.2 (64.7) | 76 / 80 (82 / 97) |
| 4000x4000 RGBA PNG to 640 px WebP | 123.5 (136.6) | 355 / 364 (318 / 324) |
| 4K JPEG to 640 px WebP with `-lowres 2` (decode at 1/4 size) | 53.2 | 56 / 58 |
| 4K JPEG to 640 px JPEG XL (libjxl) | 82.6 (91.9) | 104 / 106 (87 / 91) |
| 4K JPEG to 1920 px JPEG XL (libjxl) | 202.4 (253.3) | 500 / 505 (161 / 166) |
| **After the work: RSS the process keeps, 4K JPEG to 640 px WebP repeated in one process** | | |
| After the second job and every later one, default glibc malloc | 31.0 | |
| After four live threads took turns, one job in flight at a time | 120.9 | |
| The same two cases with glibc's mmap threshold fixed at 128 KiB | 1.1 and 2.2 | |

## What this means for a low-memory device

1. **The largest single buffer is the decoded bitmap.** It is width x height x 3 bytes, or x 4 with alpha: 23.7 MiB for 3840x2160, 17.2 MiB for 2000x3000, 61.0 MiB for 4000x4000 RGBA. `image` decodes a whole image into one such buffer and offers nothing smaller. Three other things can be as large as the bitmap or larger:
   - A progressive JPEG. The decoder keeps every DCT coefficient of the image, 2 bytes each, until the last scan: 23.7 MiB more for the 4:2:0 4K file. This doubled the decode peak (26.2 to 49.7 MiB).
   - An alpha channel in the resizer. By default `fast_image_resize` makes a premultiplied copy of the whole source: 61.0 MiB more for the PNG.
   - An alpha channel in the WebP encoder. libwebp took 27 to 32 bytes per output pixel for RGBA against 6 to 8 for RGB: 95.8 MiB for a 1920x1920 RGBA output, more than the decode and resize of the 4000x4000 source once alpha is premultiplied in place.
2. **One job at a time, pipeline as recommended, peaks at 26 to 51 MiB above baseline for the JPEG sources** and at 135 to 167 MiB for the 4000x4000 RGBA PNG. For a baseline JPEG or a PNG the peak is in the resize and is predictable from the header: bitmap, plus source width x output height x channels (the resizer's intermediate), plus the output bitmap, plus about 2 MiB. That sum was within 2 MiB of every measured resize peak. A progressive JPEG can peak earlier, in the decode, at twice its bitmap.
3. **Two changes cut the peak without a new dependency.**
   - Premultiply alpha in place before resizing, which is safe because the decoded source is thrown away next: 135.2 to 74.2 MiB for the PNG at 640 px, byte-identical output.
   - Decode from the file, not from bytes held in memory. `image`'s JPEG decoder copies the whole compressed input into its own buffer, so a JPEG held in memory is held twice while decoding; a PNG held in memory is held once beside its bitmap (8 MiB more for the 8.37 MiB PNG). For a 20 MiB upload that is 20 to 40 MiB.
4. **Scaled JPEG decoding saves most of the memory when the target is small.** 4K to 640 px went from 29.9 to 4.9 MiB above baseline and from 41 to 32 ms; 2000x3000 to 640 px from 26.1 to 10.4 MiB. A progressive JPEG keeps its coefficient store, so it saved only the bitmap: 49.5 to 26.4 MiB. It needs the target to be at most half the source in both directions, and in pure Rust only `jpeg-decoder` offers it, a crate its own README calls "in maintenance mode".
5. **A child ffmpeg is worse on both peak memory and time for JPEG sources**, and worse for the PNG too once alpha is premultiplied in place (123.5 against 74.2 MiB at 640 px). 4K to 640 px WebP: 63 to 68 MiB and 61 to 71 ms, against 30 MiB and 41 ms in-process. 43.8 MiB of that is this ffmpeg build merely starting. About 35 MiB of the child's peak is file-backed (its shared libraries), which the kernel can drop and re-read and which simultaneous ffmpeg processes share; the anonymous part, 28 MiB, is about what the in-process job costs. What the child does buy: all of its memory returns at exit, and an out-of-memory kill takes the child, not the Server.
6. **In-process memory does not come back by default.** After the second 4K job the process stayed 31 MiB above where it started, for good, as free memory glibc keeps. Each thread that has run a job keeps its own share: four live threads taking turns, never two jobs at once, left the process 121 MiB up. `malloc_trim(0)` fixed this only for the main thread. Fixing glibc's mmap threshold (one environment variable) fixed it for every thread, at a cost of roughly 10 to 25% in time per job.
7. **`image::Limits::max_alloc` is not a memory ceiling.** For JPEG and WebP it stops only an output bitmap that is too large. A progressive 4K JPEG decoded successfully under `max_alloc` = 24 MiB while the process peaked at 51.4 MiB.
8. **JPEG XL encoding with libjxl is several times heavier than WebP.** Through ffmpeg, 4K to 640 px took 83 to 92 MiB and to 1920 px 202 to 253 MiB; `cjxl` alone on a 1920x1080 PNG took 142 to 177 MiB.
9. **Every figure here is for one job.** Jobs in flight add up. Jellyfin's default is one encode per CPU core and Next.js only halves sharp's thread count; neither bounds memory on a small device.

**Recommendation (mine).** Budget **128 MiB above the Server's idle RSS for one resize, and run one at a time** (a semaphore of one around decode, resize and encode, uploads included). Five rules make the budget hold:

1. Before decoding, read the dimensions and format from the header and refuse the source (serve the original, or reject the upload) above 16.8 megapixels (4096 x 4096), above 12.5 megapixels for a JPEG, and above a 64 MiB bitmap. Set `Limits` with `max_alloc` = 64 MiB and explicit `max_image_width` and `max_image_height`; the two megapixel rules have to be the Server's own check. The JPEG rule covers a case that was computed, not measured: a progressive 4:4:4 file holds 9 bytes per pixel while decoding, about 107 MiB at 12.5 megapixels. The bitmap rule catches 16-bit PNGs, which were not measured either.
2. Premultiply alpha in place and resize with `use_alpha(false)`.
3. Keep alpha WebP outputs at or below 1920x1920: libwebp needed about 27 bytes per output pixel for them.
4. Decode from the file on disk, not from an in-memory copy.
5. Start the Server with `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072` (in the Docker image and any service unit), so each job's memory goes back to the kernel.

Under these rules the largest job measured here peaks at 111 MiB (4000x4000 RGBA PNG to 1920 px WebP), and 4K and 2000x3000 JPEGs at 26 to 51 MiB. If 128 MiB is too much, the lever is rule 1: a cap at 4K-sized RGB sources (8.3 megapixels, no alpha) brings the measured worst case to 51 MiB, at the price of refusing the 4000x4000 PNG (a progressive 4:4:4 JPEG of that size would reach about 71 MiB by the same arithmetic). Scaled decoding through `jpeg-decoder` would cut the common poster and backdrop job to 5 to 26 MiB but is a second JPEG decoder, in maintenance mode; I would add it only if 26 to 51 MiB proves too much in practice. Do not use a child ffmpeg to save memory: it does not.

## Method

**Machine.** AMD Ryzen 9 7950X3D (16 cores, 32 threads, AVX2), 61.9 GiB RAM, CachyOS (Arch-based), Linux 7.3.3-1-cachyos, x86_64, 4 KiB pages, glibc 2.44, rustc 1.99.1. System ffmpeg n9.0.2 (the Arch build, dynamically linked, with libwebp and libjxl), libjxl 0.12.0 (`cjxl`), libvips 8.18.7. This is not a low-memory device: the byte counts carry over to any 64-bit glibc Linux, the times do not. The machine was busy with unrelated work throughout (load average 19 to 74 on 32 threads), so read "best" as the fairer time and "median" as pessimistic.

**Test images.** Synthetic: fractal value noise with hard-edged regions over a colour gradient, plus per-pixel grain, written as PNG by a small Rust program and then encoded by ImageMagick 7.1.2 with `-quality 90 -sampling-factor 4:2:0`, once baseline and once with `-interlace JPEG` (progressive).

| Name in the tables | Pixels | File | Decoded bitmap |
| --- | --- | --- | --- |
| 4K JPEG | 3840x2160, 4:2:0, quality 90, baseline | 1.30 MiB | 23.7 MiB RGB |
| 4K JPEG, progressive | the same picture, progressive | 1.25 MiB | 23.7 MiB RGB |
| 2000x3000 JPEG | 2000x3000, 4:2:0, quality 90, baseline | 0.96 MiB | 17.2 MiB RGB |
| 2000x3000 JPEG, progressive | the same picture, progressive | 0.92 MiB | 17.2 MiB RGB |
| 4000x4000 RGBA PNG | 4000x4000, 8-bit RGBA, a soft-edged disc on a transparent ground | 8.37 MiB | 61.0 MiB RGBA |

**The in-process job.** A release build (`cargo build --release`, default profile) of a scratch binary, kept outside the repo in `/tmp/claude-1000/image-memory-scratch/`, with `image` 0.25.10 (`default-features = false`, features `jpeg`, `png`, `webp`, `gif`; JPEG through `zune-jpeg` 0.5.15, PNG through `png` 0.18.1, WebP through `image-webp` 0.2.4), `fast_image_resize` 6.1.0 (feature `image`, no `rayon`), `webp` 0.3.1 (`default-features = false`, libwebp through `libwebp-sys` 0.9.6) and, for scaled decoding only, `jpeg-decoder` 0.3.2 (`default-features = false`). Everything runs on one thread. One job is:

1. Decode: `ImageReader::open(path)?.with_guessed_format()?.decode()?`, default `Limits`.
2. Resize: a new `DynamicImage` of the target size and the same colour type, then `Resizer::new().resize(&src, &mut dst, &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3)))`. "640 px" and "1920 px" are widths; height keeps the aspect ratio.
3. Drop the resizer and the source bitmap.
4. Encode into a `Vec`: JPEG with `JpegEncoder::new_with_quality(&mut out, 80)` (an RGBA image is converted with `to_rgb8` first), or WebP with `webp::Encoder::from_rgb(..)` or `from_rgba(..)` and `.encode(75.0)`.

EXIF orientation is not applied and no ICC profile is carried over.

**How peak RSS was read.** One process per job, so a peak belongs to one job. `/usr/bin/time` is not installed here, so the binary reads the kernel's own counters:

- The peak is `VmHWM` from `/proc/self/status`, the kernel's high-water mark of RSS ([proc documentation](https://docs.kernel.org/filesystems/proc.html)). The binary resets it at the start of each stage by writing `5` to `/proc/self/clear_refs` (same page) and reads it at the end of the stage, so decode, resize and encode each get their own peak; the job's peak is the largest.
- The current RSS at each stage boundary is `Rss` from `/proc/self/smaps_rollup`, which is exact. A job's peak is taken as the larger of the two readings, because on this kernel the high-water mark read 0.6 to 1.5 MiB below an exact RSS reading taken at the same moment.
- Memory runs were pinned to one CPU with `sched_setaffinity`, which made the high-water mark steadier. Five pinned runs per job in each of three sessions and one in a fourth; the tables show the highest. Across all runs of one job the peak varied by 0.4 MiB at the median and 1.1 MiB at most. Treat every figure as good to about 1 MiB.
- Transparent huge pages are set to `always` on this machine. The binary turns them off for itself and its children with `prctl(PR_SET_THP_DISABLE)`, so the figures are for 4 KiB pages. Two jobs repeated with huge pages left on gave the same peaks.
- No run touched swap (`VmSwap` stayed 0).
- "Baseline" is the exact RSS of the same binary started with nothing to do: median 3.3 MiB (3,400 kB; 3,264 to 3,496 kB over 36 runs) while the main tables were measured. It is nearly all file-backed pages of the binary and libc, and it moves with the state of the page cache: later the same evening it read 2.6 to 3.0 MiB. Each set of runs is compared with the idle reading of its own time.
- "RSS after drop" is the exact RSS once the source, the output image and the encoded bytes are all dropped, in a fresh process after its first job.

**How times were read.** `Instant` around each stage, so the memory readings between stages are not counted. Runs for timing were not pinned. Each job was timed 15 times in two sessions and 25 in a third; the tables give the fastest single run with its own decode / resize / encode split, and the median of the session that run came from.

**The child-process job.** The same binary spawns the command, waits with `wait4` and reports `ru_maxrss`, the number `/usr/bin/time -v` calls "Maximum resident set size", and the wall time from spawn to exit. Seven runs with default threads; for one thread, five runs pinned to one CPU for memory and seven unpinned for time; all of it twice. The commands:

- WebP: `ffmpeg -hide_banner -loglevel error -nostdin -y -i SRC -vf scale=W:-2 -c:v libwebp -quality 75 OUT.webp`
- JPEG: the same with `-q:v 4 -frames:v 1 -update 1 OUT.jpg`
- JPEG XL: the same with `-c:v libjxl -distance 1.0 -frames:v 1 OUT.jxl`
- "One thread" adds `-threads 1 -filter_threads 1` before `-i` and again after the filter. "Default threads" lets ffmpeg size its pools from the 32 CPUs.
- `scale` uses ffmpeg's default scaler (bicubic), not Lanczos.

Two traps in this method, both checked:

- After `exec`, `ru_maxrss` cannot read lower than the RSS of the process image that called `exec`. Launched from the 3 MiB test binary the floor is 3.0 MiB (measured with `/bin/true`), far below any ffmpeg figure. Launched from a 14 MiB Python script it was 14.1 MiB, which is why the in-process figures use `VmHWM` and not `getrusage`.
- `ru_maxrss` is the same lagging high-water mark. A sampler reading the child's `/proc/PID/status` in a loop saw 62.7 MiB where `ru_maxrss` said 61.9 MiB.

## Full tables

### In-process: decode, resize, encode

"Peak falls in" names the stage whose own peak is the job's peak. The three times are those of the fastest run.

| Source | Job | Output | Peak RSS, MiB | Above baseline, MiB | Peak falls in | Best, ms | Median, ms | Decode / resize / encode, ms | RSS after drop, MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4K JPEG | decode only | - | 29.5 | 26.2 | decode | 24 | 26 | 24 / 0 / 0 | 4.0 |
| 4K JPEG | to 640 px, no encode | 640x360 | 33.3 | 29.9 | resize | 30 | 33 | 24 / 6 / 0 | 5.3 |
| 4K JPEG | to 640 px, JPEG | 640x360 | 33.3 | 30.0 | resize | 34 | 36 | 24 / 6 / 4 | 5.6 |
| 4K JPEG | to 640 px, WebP | 640x360 | 33.3 | 29.9 | resize | 41 | 46 | 24 / 6 / 11 | 7.2 |
| 4K JPEG | to 1920 px, no encode | 1920x1080 | 46.6 | 43.3 | resize | 34 | 40 | 24 / 11 / 0 | 5.0 |
| 4K JPEG | to 1920 px, JPEG | 1920x1080 | 46.6 | 43.3 | resize | 66 | 70 | 24 / 11 / 30 | 4.7 |
| 4K JPEG | to 1920 px, WebP | 1920x1080 | 46.5 | 43.2 | resize | 118 | 128 | 25 / 11 / 82 | 17.2 |
| 4K JPEG, progressive | decode only | - | 53.1 | 49.7 | decode | 65 | 70 | 65 / 0 / 0 | 4.4 |
| 4K JPEG, progressive | to 640 px, no encode | 640x360 | 53.2 | 49.9 | decode | 74 | 86 | 68 / 6 / 0 | 9.3 |
| 4K JPEG, progressive | to 640 px, JPEG | 640x360 | 53.1 | 49.8 | decode | 77 | 86 | 68 / 6 / 4 | 9.2 |
| 4K JPEG, progressive | to 640 px, WebP | 640x360 | 52.8 | 49.5 | decode | 86 | 100 | 69 / 6 / 11 | 9.8 |
| 4K JPEG, progressive | to 1920 px, no encode | 1920x1080 | 53.0 | 49.7 | decode | 79 | 86 | 65 / 14 / 0 | 22.7 |
| 4K JPEG, progressive | to 1920 px, JPEG | 1920x1080 | 53.0 | 49.7 | decode | 110 | 116 | 65 / 13 / 32 | 22.5 |
| 4K JPEG, progressive | to 1920 px, WebP | 1920x1080 | 53.1 | 49.7 | decode | 169 | 203 | 70 / 12 / 86 | 23.3 |
| 2000x3000 JPEG | decode only | - | 22.2 | 18.9 | decode | 21 | 22 | 20 / 0 / 0 | 4.3 |
| 2000x3000 JPEG | to 640 px, no encode | 640x960 | 29.4 | 26.1 | resize | 29 | 33 | 22 / 7 / 0 | 4.9 |
| 2000x3000 JPEG | to 640 px, JPEG | 640x960 | 29.4 | 26.1 | resize | 35 | 36 | 19 / 7 / 9 | 4.9 |
| 2000x3000 JPEG | to 640 px, WebP | 640x960 | 29.4 | 26.1 | resize | 56 | 61 | 22 / 6 / 27 | 8.7 |
| 2000x3000 JPEG | to 1920 px, no encode | 1920x2880 | 54.6 | 51.2 | resize | 37 | 42 | 18 / 19 / 0 | 4.9 |
| 2000x3000 JPEG | to 1920 px, JPEG | 1920x2880 | 54.6 | 51.3 | resize | 126 | 159 | 20 / 21 / 85 | 5.5 |
| 2000x3000 JPEG | to 1920 px, WebP | 1920x2880 | 54.6 | 51.3 | resize | 272 | 281 | 18 / 19 / 236 | 15.6 |
| 2000x3000 JPEG, progressive | decode only | - | 39.3 | 36.0 | decode | 48 | 49 | 48 / 0 / 0 | 4.2 |
| 2000x3000 JPEG, progressive | to 640 px, no encode | 640x960 | 39.3 | 36.0 | decode | 54 | 60 | 47 / 7 / 0 | 12.1 |
| 2000x3000 JPEG, progressive | to 640 px, JPEG | 640x960 | 39.1 | 35.8 | decode | 63 | 69 | 47 / 6 / 9 | 12.0 |
| 2000x3000 JPEG, progressive | to 640 px, WebP | 640x960 | 39.3 | 35.9 | decode | 78 | 88 | 47 / 6 / 26 | 12.6 |
| 2000x3000 JPEG, progressive | to 1920 px, no encode | 1920x2880 | 54.6 | 51.3 | resize | 67 | 72 | 47 / 20 / 0 | 5.1 |
| 2000x3000 JPEG, progressive | to 1920 px, JPEG | 1920x2880 | 54.6 | 51.3 | resize | 153 | 158 | 48 / 20 / 84 | 5.3 |
| 2000x3000 JPEG, progressive | to 1920 px, WebP | 1920x2880 | 54.6 | 51.3 | resize | 304 | 317 | 49 / 21 / 235 | 15.8 |
| 4000x4000 RGBA PNG | decode only | - | 65.1 | 61.7 | decode | 80 | 83 | 80 / 0 / 0 | 4.0 |
| 4000x4000 RGBA PNG | to 640 px, no encode | 640x640 | 138.6 | 135.2 | resize | 115 | 123 | 79 / 36 / 0 | 4.9 |
| 4000x4000 RGBA PNG | to 640 px, JPEG | 640x640 | 138.5 | 135.2 | resize | 120 | 129 | 78 / 35 / 6 | 6.0 |
| 4000x4000 RGBA PNG | to 640 px, WebP | 640x640 | 138.5 | 135.2 | resize | 150 | 155 | 78 / 34 / 37 | 16.9 |
| 4000x4000 RGBA PNG | to 1920 px, no encode | 1920x1920 | 170.4 | 167.1 | resize | 130 | 138 | 79 / 52 / 0 | 4.6 |
| 4000x4000 RGBA PNG | to 1920 px, JPEG | 1920x1920 | 170.6 | 167.3 | resize | 185 | 192 | 80 / 48 / 58 | 15.3 |
| 4000x4000 RGBA PNG | to 1920 px, WebP | 1920x1920 | 170.4 | 167.1 | resize | 569 | 574 | 81 / 51 / 436 | 35.5 |

Reading the table:

- **Decode only.** The peak is the bitmap plus the compressed file plus, for a progressive JPEG, the coefficient store.
- **Resize.** The peak is the bitmap, the resizer's intermediate and the output together: 23.7 + 4.0 + 0.7 MiB for 4K to 640 px, 23.7 + 11.9 + 5.9 MiB for 4K to 1920 px, 17.2 + 16.5 + 15.8 MiB for 2000x3000 to 1920x2880 (which is hardly a reduction). The measured peaks are those sums plus 1.5 to 2 MiB.
- **Encode.** With the source dropped first, encoding never set the peak for an RGB image. JPEG encoding took 0.1 to 0.6 MiB of its own; WebP is in the next tables.
- **Progressive JPEG.** At 640 px the peak is in decode and is the same whatever follows. Decoding also takes 2.3 to 2.7 times as long.
- **RSS after drop** is 4 to 36 MiB after a single job, against a 3.3 MiB baseline. What is kept is free memory inside glibc; see [Whether the memory comes back](#whether-the-memory-comes-back).

Resize filter, 4K JPEG to 640 px, no encode: the filter changes the time, not the memory.

| Filter | Peak RSS, MiB | Resize alone, ms |
| --- | --- | --- |
| Lanczos3 | 33.3 | 5.9 |
| CatmullRom | 33.0 | 4.7 |
| Bilinear | 32.9 | 3.1 |
| Box | 32.8 | 2.4 |

### What the encoders take

"RSS before encode" is read after the source and the resizer are dropped, so it is the baseline plus the resized image plus whatever glibc kept. The encoder's own share is the encode stage's peak minus that.

| Encoder | Output | RSS before encode, MiB | Peak during encode, MiB | Encoder's own, MiB | Bytes per output pixel |
| --- | --- | --- | --- | --- | --- |
| JPEG q80, RGB | 640x360 | 5.6 | 5.6 | 0.1 | 0.4 |
| JPEG q80, RGB | 1920x1080 | 10.7 | 11.0 | 0.3 | 0.2 |
| JPEG q80, RGB | 1920x2880 | 20.9 | 21.5 | 0.6 | 0.1 |
| WebP q75, RGB | 640x360 | 5.6 | 7.3 | 1.7 | 7.9 |
| WebP q75, RGB | 640x960 | 6.5 | 10.7 | 4.2 | 7.2 |
| WebP q75, RGB | 1920x1080 | 10.8 | 23.2 | 12.4 | 6.3 |
| WebP q75, RGB | 1920x2880 | 20.9 | 52.0 | 31.1 | 5.9 |
| WebP q75, RGBA | 640x640 | 6.5 | 18.9 | 12.4 | 31.7 |
| WebP q75, RGBA | 1920x1920 | 18.7 | 114.6 | 95.8 | 27.3 |

### The 4000x4000 RGBA PNG: alpha handling and WebP settings

"Premultiplied in place" is `MulDiv::new().multiply_alpha_inplace(&mut src)`, then the resize with `.use_alpha(false)`, then `MulDiv::new().divide_alpha_inplace(&mut dst)`. Its WebP output is byte-identical to the default path's. `method` and `alpha_compression` are fields of `webp::WebPConfig`, passed to `Encoder::encode_advanced`; the default `method` is 4.

| Output | Alpha handling and WebP settings | Peak RSS, MiB | Above baseline, MiB | Peak falls in | Resize stage peak, MiB | Encode stage peak, MiB | WebP bytes | Best, ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 640x640 | resizer default (`mul_div_alpha: true`), `encode(75.0)` | 138.5 | 135.2 | resize | 138.5 | 18.8 | 27,826 | 150 |
| 640x640 | premultiplied in place, `encode(75.0)` | 77.6 | 74.2 | resize | 77.6 | 18.9 | 27,826 | 134 |
| 640x640 | in place, `method = 2` | 77.6 | 74.2 | resize | 77.6 | 16.5 | 29,420 | 119 |
| 640x640 | in place, `alpha_compression = 0` | 77.4 | 74.1 | resize | 77.4 | 11.3 | 426,344 | 113 |
| 1920x1920 | resizer default (`mul_div_alpha: true`), `encode(75.0)` | 170.4 | 167.1 | resize | 170.4 | 114.7 | 128,716 | 569 |
| 1920x1920 | premultiplied in place, `encode(75.0)` | 114.6 | 111.2 | encode | 109.3 | 114.6 | 128,716 | 560 |
| 1920x1920 | in place, `method = 2` | 109.3 | 106.0 | resize | 109.3 | 96.2 | 128,548 | 366 |
| 1920x1920 | in place, `alpha_compression = 0` | 109.3 | 106.0 | resize | 109.3 | 56.6 | 3,753,074 | 241 |

- With the resizer's defaults the peak is 61.0 MiB (source) + 61.0 MiB (premultiplied copy) + 9.8 or 29.3 MiB (intermediate) + the output.
- Premultiplying in place removes the copy. At 1920 px the WebP encoder then becomes the peak: 95.8 MiB of its own for 3.7 megapixels of RGBA.
- `method = 2` lowered the encoder's share to 77.5 MiB at 1920 px for a file of the same size. `alpha_compression = 0` lowered it to 38 MiB but stores the alpha plane raw: 3.75 MB instead of 0.13 MB, which defeats the purpose.

### Other decoders and inputs, decode only

| Source (file size) | Decoder and input | Bitmap, MiB | Peak RSS, MiB | Above baseline, MiB | Above the bitmap, MiB | Best, ms | Median, ms |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 4K JPEG, baseline (1.30 MiB) | `image`, file | 23.7 | 29.5 | 26.2 | 2.5 | 24 | 26 |
| 4K JPEG, baseline (1.30 MiB) | `image`, bytes already in memory | 23.7 | 30.6 | 27.3 | 3.6 | 25 | 28 |
| 4K JPEG, progressive (1.25 MiB) | `image`, file | 23.7 | 53.1 | 49.7 | 26.0 | 65 | 70 |
| 2000x3000 JPEG, baseline (0.96 MiB) | `image`, file | 17.2 | 22.2 | 18.9 | 1.8 | 21 | 22 |
| 2000x3000 JPEG, progressive (0.92 MiB) | `image`, file | 17.2 | 39.3 | 36.0 | 18.8 | 48 | 49 |
| 4K RGB PNG (15.7 MiB) | `image`, file | 23.7 | 27.8 | 24.5 | 0.7 | 44 | 46 |
| 4000x4000 RGBA PNG (8.37 MiB) | `image`, file | 61.0 | 65.1 | 61.7 | 0.7 | 80 | 83 |
| 4000x4000 RGBA PNG (8.37 MiB) | `image`, bytes already in memory | 61.0 | 73.1 | 69.8 | 8.7 | 81 | 85 |
| 4K lossy WebP (1.36 MiB) | `image`, file | 23.7 | 40.1 | 36.8 | 13.1 | 140 | 148 |
| 4000x4000 lossless RGBA WebP (4.20 MiB) | `image`, file | 61.0 | 67.2 | 63.9 | 2.8 | 121 | 125 |
| 4K JPEG, baseline | `jpeg-decoder`, file, full size | 23.7 | 39.4 | 36.1 | 12.3 | 28 | 31 |
| 4K JPEG, progressive | `jpeg-decoder`, file, full size | 23.7 | 63.2 | 59.9 | 36.2 | 84 | 92 |
| 2000x3000 JPEG, baseline | `jpeg-decoder`, file, full size | 17.2 | 29.5 | 26.2 | 9.0 | 20 | 22 |
| 2000x3000 JPEG, progressive | `jpeg-decoder`, file, full size | 17.2 | 46.7 | 43.3 | 26.2 | 58 | 62 |

- "Above the bitmap" is what decoding holds beside its output at the peak: the decoder's copy of the input plus about 1 MiB for a baseline JPEG, the same plus the coefficient store for a progressive one, under 1 MiB for PNG, the Y, U and V planes (11.9 MiB) plus the compressed data for lossy WebP.
- Decoding from bytes already in memory adds the caller's copy of the file: 1.1 MiB for the JPEG, 8 MiB for the PNG.
- `jpeg-decoder` at full size holds all three colour planes as well as the output, so it is 10 MiB heavier than `zune-jpeg` for the 4K file. It is only worth having for its scaled mode.

### Scaled JPEG decoding

`jpeg-decoder` 0.3.2, asked for the target size with `Decoder::scale`, picks 1/4 for 4K to 640 px (960x540), 1/2 for 2000x3000 to 640 px (1000x1500, because 1/4 would give 500 px) and 1/2 for 4K to 1920 px (exactly 1920x1080, so no resize is left to do). The result then goes through the same `fast_image_resize` and `webp` steps. The two WebP files for the 4K source compare at 41 dB PSNR.

| Source | Decoder | Decoded at | Output | Peak RSS, MiB | Above baseline, MiB | Peak falls in | Best, ms | Median, ms | Decode / resize / encode, ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4K JPEG | `image` (zune-jpeg) | 3840x2160 | 640x360 WebP | 33.3 | 29.9 | resize | 41 | 46 | 24 / 6 / 11 |
| 4K JPEG | `jpeg-decoder` with `scale` | 960x540 | 640x360 WebP | 8.2 | 4.9 | resize | 32 | 34 | 17 / 2 / 13 |
| 4K JPEG, progressive | `image` (zune-jpeg) | 3840x2160 | 640x360 WebP | 52.8 | 49.5 | decode | 86 | 100 | 69 / 6 / 11 |
| 4K JPEG, progressive | `jpeg-decoder` with `scale` | 960x540 | 640x360 WebP | 29.7 | 26.4 | decode | 81 | 103 | 67 / 2 / 13 |
| 2000x3000 JPEG | `image` (zune-jpeg) | 2000x3000 | 640x960 WebP | 29.4 | 26.1 | resize | 56 | 61 | 22 / 6 / 27 |
| 2000x3000 JPEG | `jpeg-decoder` with `scale` | 1000x1500 | 640x960 WebP | 13.7 | 10.4 | resize | 43 | 45 | 13 / 3 / 27 |
| 2000x3000 JPEG, progressive | `image` (zune-jpeg) | 2000x3000 | 640x960 WebP | 39.3 | 35.9 | decode | 78 | 88 | 47 / 6 / 26 |
| 2000x3000 JPEG, progressive | `jpeg-decoder` with `scale` | 1000x1500 | 640x960 WebP | 27.2 | 23.9 | decode | 82 | 92 | 51 / 4 / 27 |
| 4K JPEG | `image` (zune-jpeg) | 3840x2160 | 1920x1080 WebP | 46.5 | 43.2 | resize | 118 | 128 | 25 / 11 / 82 |
| 4K JPEG | `jpeg-decoder` with `scale` | 1920x1080 | 1920x1080 WebP | 22.3 | 18.9 | encode | 106 | 114 | 18 / 2 / 86 |
| 4K JPEG, progressive | `image` (zune-jpeg) | 3840x2160 | 1920x1080 WebP | 53.1 | 49.7 | decode | 169 | 203 | 70 / 12 / 86 |
| 4K JPEG, progressive | `jpeg-decoder` with `scale` | 1920x1080 | 1920x1080 WebP | 36.5 | 33.2 | decode | 155 | 174 | 65 / 2 / 88 |

- The saving is the full-size bitmap and the resizer's intermediate. For a progressive file the full-size coefficient store remains, so the saving is about half.
- At 640 px the scaled path took 21 ms with JPEG output and 16 ms with no encode for the 4K file, 4.9 MiB above baseline in both.
- `jpeg-decoder` starts worker threads of its own for any image larger than 128x128, even without its `rayon` feature. Left free to use other CPUs, its peaks read 0.2 to 1.3 MiB higher than pinned to one.

### Whether the memory comes back

The job is 4K JPEG to 640 px WebP unless stated, repeated in one process. All figures but the first column are exact RSS in MiB above the process's own starting RSS, so they compare with the "above baseline" figures elsewhere. This set was run last, when an idle process read 2.6 to 2.9 MiB.

| Run | Starting RSS | Peak, first job | After first job | Peak, later jobs | After the second and every later job |
| --- | --- | --- | --- | --- | --- |
| Default glibc, main thread, 6 jobs | 2.6 | 29.3 | 3.2 | 31.0 | 31.0 |
| Default glibc, a spawned thread, 6 jobs | 2.9 | 29.2 | 2.9 | 30.7 | 30.7, and 30.7 after the thread exits |
| `malloc_trim(0)` after each job, main thread | 2.9 | 29.2 | 0.7 | 30.7 | 0.8 |
| `malloc_trim(0)` after each job, a spawned thread | 2.8 | 29.2 | 1.9 | 30.7 | 29.7 |
| mmap threshold fixed, main thread | 2.8 | 29.1 | 1.1 | 29.5 | 1.1 |
| mmap threshold fixed, a spawned thread | 2.9 | 29.0 | 1.1 | 29.4 | 1.1 |
| Default glibc, 4 live threads taking turns, 3 jobs each | 2.9 | 29.2 | 3.0 | 120.9 | 120.9 |
| mmap threshold fixed, 4 live threads taking turns | 2.8 | 29.1 | 1.2 | 30.6 | 2.2 |
| 4K progressive JPEG, default glibc, a spawned thread | 2.8 | 49.1 | 5.8 | 53.3 | 53.3 |
| 4K progressive JPEG, mmap threshold fixed, a spawned thread | 2.9 | 49.1 | 1.1 | 49.5 | 1.1 |
| 4000x4000 PNG (alpha in place), default glibc, a spawned thread | 2.8 | 73.7 | 13.2 | 75.8 | 14.7 |
| 4000x4000 PNG (alpha in place), mmap threshold fixed, a spawned thread | 2.9 | 73.2 | 1.1 | 73.6 | 1.2 |

"mmap threshold fixed" is the environment variable `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072`. `MALLOC_MMAP_THRESHOLD_=131072` and `MALLOC_TRIM_THRESHOLD_=131072` gave the same result on the main thread; `MALLOC_ARENA_MAX=1` changed nothing there.

- **Default.** The first job's buffers come from `mmap` and go back. From the second job on they come from the heap, and the freed heap stays in the process: the process then sits at its own peak for good. glibc's `mallinfo2` showed 30.1 MiB of heap, all of it free.
- **Threads.** Each thread that ran a job kept its own 30 MiB. With four live threads taking turns, never two jobs at once, the process ended 120.9 MiB up. The memory stayed after a thread exited.
- **`malloc_trim(0)`** returned everything when the jobs ran on the main thread and 1 MiB when they ran on any other thread, which is where a server's blocking work runs.
- **A fixed threshold** returned everything on every thread. Its cost, timed with 20 jobs in one process, three rounds each: the 4K to 640 px job went from a median of 48 to 50 ms to 59 to 63 ms, the PNG job from 178 to 189 ms to 202 to 214 ms, 4K to 1920 px from 131 to 182 ms to 162 to 187 ms. The extra time is the kernel handing out and zeroing fresh pages for every job.
- **Buffers over 32 MiB** always go back, whatever the setting: the PNG's 61 MiB bitmap is returned by default, and what the PNG job keeps (14.7 MiB) is the WebP encoder's smaller allocations.

### The same jobs in a child ffmpeg

| Source | Output | One thread: peak RSS, MiB | best / median, ms | Default threads: peak RSS, MiB | best / median, ms |
| --- | --- | --- | --- | --- | --- |
| 4K JPEG | 640 px WebP | 63.4 | 61 / 62 | 68.3 | 71 / 72 |
| 4K JPEG | 640 px JPEG | 64.1 | 50 / 52 | 72.7 | 63 / 65 |
| 4K JPEG | 1920 px WebP | 65.6 | 129 / 132 | 72.4 | 140 / 145 |
| 4K JPEG | 1920 px JPEG | 71.7 | 59 / 60 | 86.1 | 71 / 74 |
| 4K JPEG, progressive | 640 px WebP | 86.9 | 77 / 81 | 91.9 | 86 / 90 |
| 4K JPEG, progressive | 640 px JPEG | 88.1 | 69 / 70 | 96.9 | 80 / 84 |
| 4K JPEG, progressive | 1920 px WebP | 89.6 | 155 / 162 | 96.2 | 158 / 161 |
| 4K JPEG, progressive | 1920 px JPEG | 95.7 | 87 / 92 | 109.7 | 92 / 98 |
| 2000x3000 JPEG | 640 px WebP | 60.2 | 76 / 80 | 64.7 | 82 / 97 |
| 2000x3000 JPEG | 640 px JPEG | 62.0 | 57 / 60 | 70.9 | 62 / 71 |
| 2000x3000 JPEG | 1920 px WebP | 67.3 | 277 / 302 | 74.1 | 296 / 319 |
| 2000x3000 JPEG | 1920 px JPEG | 81.5 | 74 / 76 | 105.3 | 88 / 96 |
| 4000x4000 RGBA PNG | 640 px WebP | 123.5 | 355 / 364 | 136.6 | 318 / 324 |
| 4000x4000 RGBA PNG | 640 px JPEG | 123.5 | 292 / 293 | 141.1 | 304 / 308 |
| 4000x4000 RGBA PNG | 1920 px WebP | 134.6 | 785 / 836 | 151.6 | 714 / 736 |
| 4000x4000 RGBA PNG | 1920 px JPEG | 137.6 | 300 / 310 | 166.5 | 318 / 336 |

- `ffmpeg -version` alone peaks at 43.8 MiB and takes 25 ms: dynamic linking against this build's many libraries. At one thread the image work adds 16 to 38 MiB for the baseline JPEG sources, 43 to 52 MiB for the progressive one and 80 to 94 MiB for the PNG.
- Default threads cost 4 to 29 MiB more than one thread and are slower for the JPEG sources; for a single still, pass `-threads 1 -filter_threads 1`.
- With transparent huge pages left on, the default-threads 4K to 640 px WebP job peaked at 70.8 MiB instead of 68.3.

Split of the child's peak into anonymous and file-backed pages, from sampling `/proc/PID/status` (one thread, 640 px WebP):

| Child | Peak RSS seen, MiB | Anonymous, MiB | File-backed, MiB |
| --- | --- | --- | --- |
| `ffmpeg -version` | 43.9 | 13.4 | 30.5 |
| 4K JPEG | 62.7 | 27.9 | 34.8 |
| 2000x3000 JPEG | 59.7 | 24.9 | 34.8 |
| 4000x4000 RGBA PNG | 123.6 | 91.4 | 32.2 |

ffmpeg's JPEG decoder can itself decode at reduced size with `-lowres` before `-i` (1 = 1/2, 2 = 1/4, 3 = 1/8). One thread:

| Source | Output | Full decode: peak RSS, MiB | best, ms | With `-lowres`: peak RSS, MiB | best, ms |
| --- | --- | --- | --- | --- | --- |
| 4K JPEG | 640 px WebP | 63.4 | 61 | 53.2 (`-lowres 2`) | 56 |
| 4K JPEG | 640 px JPEG | 64.1 | 50 | 54.6 (`-lowres 2`) | 45 |
| 2000x3000 JPEG | 640 px WebP | 60.2 | 76 | 54.1 (`-lowres 1`) | 79 |
| 2000x3000 JPEG | 640 px JPEG | 62.0 | 57 | 56.0 (`-lowres 1`) | 51 |

### JPEG XL

No Rust encoder was measured; these are libjxl 0.12.0 at its default effort 7 and distance 1.0, through the system ffmpeg and through `cjxl`.

| Source | Output | One thread: peak RSS, MiB | best / median, ms | Default threads: peak RSS, MiB | best / median, ms |
| --- | --- | --- | --- | --- | --- |
| 4K JPEG | 640 px JPEG XL | 82.6 | 104 / 106 | 91.9 | 87 / 91 |
| 4K JPEG | 1920 px JPEG XL | 202.4 | 500 / 505 | 253.3 | 161 / 166 |
| 2000x3000 JPEG | 640 px JPEG XL | 104.8 | 210 / 233 | 125.6 | 121 / 140 |
| 2000x3000 JPEG | 1920 px JPEG XL | 337.9 | 1310 / 1591 | 423.0 | 302 / 364 |
| 4000x4000 RGBA PNG | 640 px JPEG XL | 148.2 | 429 / 450 | 204.5 | 364 / 480 |
| 4000x4000 RGBA PNG | 1920 px JPEG XL | 399.3 | 1657 / 1733 | 524.7 | 600 / 672 |

`cjxl IN.png OUT.jxl -d 1`, on PNGs already at the target size, so this is the encoder plus a small PNG decode:

| Job | Peak RSS, MiB | Best / median, ms |
| --- | --- | --- |
| `cjxl --version` | 6.1 | 2 / 2 |
| 640x360, default threads | 40.4 | 37 / 38 |
| 640x360, `--num_threads=0` | 26.5 | 71 / 76 |
| 1920x1080, default threads | 176.9 | 158 / 162 |
| 1920x1080, `--num_threads=0` | 142.4 | 540 / 603 |

At 1920x1080 that is 69 to 86 bytes per output pixel above `cjxl`'s start-up, against 6 for libwebp. The ffmpeg figures for the 2000x3000 source at 1920 px (338 to 423 MiB) are for a 1920x2880 output.

### libvips, for reference

libvips is the engine under sharp and Next.js. It is not a candidate here (the earlier survey rejects it as a system library); `vips thumbnail SRC 'OUT.webp[Q=75]' W` was run to see what its streaming design and JPEG shrink-on-load achieve. `vips --version` alone peaks at 34.6 MiB.

| Source | Output | `VIPS_CONCURRENCY=1`: peak RSS, MiB | best / median, ms | Default (32 threads): peak RSS, MiB | best / median, ms |
| --- | --- | --- | --- | --- | --- |
| 4K JPEG | 640 px WebP | 48.9 | 77 / 81 | 102.1 | 100 / 104 |
| 4K JPEG | 1920 px WebP | 64.0 | 188 / 191 | 165.1 | 246 / 262 |
| 4K JPEG, progressive | 640 px WebP | 72.5 | 128 / 132 | 125.6 | 156 / 161 |
| 4K JPEG, progressive | 1920 px WebP | 87.4 | 224 / 232 | 195.8 | 213 / 230 |
| 2000x3000 JPEG | 640 px WebP | 44.8 | 71 / 73 | 71.7 | 83 / 96 |
| 2000x3000 JPEG | 1920 px WebP | 58.7 | 189 / 192 | 126.6 | 210 / 221 |
| 4000x4000 RGBA PNG | 640 px WebP | 74.2 | 330 / 338 | 184.4 | 331 / 376 |
| 4000x4000 RGBA PNG | 1920 px WebP | 143.8 | 876 / 946 | 439.2 | 925 / 1208 |

At one thread libvips took 14 MiB above its own start-up for 4K to 640 px, and its anonymous memory at the peak was 12.9 MiB. With its default thread pool (32 threads here) the peak doubled or tripled.

## Facts from primary sources

### a. Scaled JPEG decoding

- **`image` 0.25.10 has none.** Its `JpegDecoder` has one inherent method, `new` ([docs](https://docs.rs/image/0.25.10/image/codecs/jpeg/struct.JpegDecoder.html)). `image` 0.24, which decoded JPEG with `jpeg-decoder`, had `JpegDecoder::scale` with "The currently implemented scale factors are 1/8, 1/4, 1/2 and 1" ([0.24.9 docs](https://docs.rs/image/0.24.9/image/codecs/jpeg/struct.JpegDecoder.html#method.scale)); the method went away with the move to `zune-jpeg`.
- **`zune-jpeg` 0.5.15 has none.** `zune_core`'s `DecoderOptions` has limits, colour space and strictness settings and nothing for scale ([docs](https://docs.rs/zune-core/0.5.3/zune_core/options/struct.DecoderOptions.html)). The decoder does have 4x4 and 1x1 IDCT functions, but they are shortcuts for blocks whose other coefficients are zero and still write a full 8x8 block ([source, `idct.rs`](https://docs.rs/crate/zune-jpeg/0.5.15/source/src/idct.rs)). Two requests for it are open: [zune-image #103](https://github.com/etemesi254/zune-image/issues/103) (2023-03-13) and [#434](https://github.com/etemesi254/zune-image/issues/434) (2026-08-18). The newest pre-release, 0.5.16-rc3 of 2026-10-19, has no such option either (its source was searched).
- **`jpeg-decoder` 0.3.2 has it.** `Decoder::scale(requested_width, requested_height)` "efficiently scales the image by the smallest supported scale factor that produces an image larger than or equal to the requested size in at least one axis" ([docs](https://docs.rs/jpeg-decoder/0.3.2/jpeg_decoder/struct.Decoder.html#method.scale)). It is invoked like this:

  ```rust
  let mut decoder = jpeg_decoder::Decoder::new(BufReader::new(File::open(path)?));
  let (width, height) = decoder.scale(target_width, target_height)?; // 960x540 for 4K and a 640x360 target
  let pixels = decoder.decode()?; // RGB24 for a YCbCr file
  ```

  Licence MIT OR Apache-2.0. A scratch crate depending on it passed `cargo deny check licenses advisories sources` (cargo-deny 0.20.2) against a copy of this repo's `deny.toml`, with and without its default `rayon` feature; it adds one crate that `Cargo.lock` does not already hold. OSV lists no advisory for it. Against that: its README opens with "This crate is in maintenance mode as image-rs seeks to move over to the zune-jpeg crate" ([README](https://github.com/image-rs/jpeg-decoder/blob/master/README.md)); the last release is 0.3.2 of 2025-06-21, none in the last 12 months, and the last push to the repository is that same day ([crates.io](https://crates.io/crates/jpeg-decoder)). This repo fails a dependency on any RustSec advisory, "unmaintained" included, so the crate passes today and would stop passing the day it is flagged. Note "in at least one axis": pass a target with the source's aspect ratio. It also starts worker threads for any image over 128x128 ([source, `decoder.rs` lines 243 to 260, and `worker/mod.rs`](https://docs.rs/crate/jpeg-decoder/0.3.2/source/src/decoder.rs)).
- **`turbojpeg` 1.5.1 has it** (`Decompressor::set_scaling_factor`, [docs](https://docs.rs/turbojpeg/1.5.1/turbojpeg/struct.Decompressor.html#method.set_scaling_factor)). The earlier survey did not cover this crate. It declares `Unlicense OR MIT` and passed the same cargo-deny run, adding 3 new crates (`turbojpeg`, `turbojpeg-sys`, `gcd`). But it is libjpeg-turbo in C: "you will need CMake, a C compiler and NASM to build the library from source" ([README](https://github.com/honzasp/rust-turbojpeg)), on all five release targets; and the libjpeg-turbo it vendors is under "The IJG (Independent JPEG Group) License" and "The Modified (3-clause) BSD License" ([vendored LICENSE.md](https://docs.rs/crate/turbojpeg-sys/1.2.0/source/libjpeg-turbo/LICENSE.md)), which the crate's metadata does not say. IJG is the licence `deny.toml` rejects for `mozjpeg`.
- **`mozjpeg`** declares `IJG` and fails `deny.toml`, as the earlier survey found.
- **`zenjpeg` 0.8.4** (AGPL-3.0-only or commercial; passed the cargo-deny run; 24 new crates) describes a row-by-row "scanline reader" and memory limits in its README but no scaled decode; none was found in its source either ([crates.io](https://crates.io/crates/zenjpeg)). Not measured.
- **ffmpeg** has `-lowres`: "Decode at 1= 1/2, 2=1/4, 3=1/8 resolutions" ([ffmpeg-codecs](https://ffmpeg.org/ffmpeg-codecs.html)); measured above.

### b. What `image::Limits` bounds

- The struct has three fields. `max_image_width` and `max_image_height` are "strict". `max_alloc` is "The maximum allowed sum of allocations allocated by the decoder at any one time excluding allocator overhead. This limit is non-strict by default and some decoders may ignore it. The bytes required to store the output image count towards this value. The default is 512MiB." ([docs](https://docs.rs/image/0.25.10/image/struct.Limits.html), [source](https://docs.rs/crate/image/0.25.10/source/src/io/limits.rs)). `ImageReader` starts from `Limits::default()`, so by default a bitmap of up to 512 MiB is allowed and there is no width or height cap.
- **The output buffer is covered for every format.** `ImageReader::decode` calls `limits.reserve(decoder.total_bytes())` before it allocates ([source, `image_reader_type.rs` lines 310 to 323](https://docs.rs/crate/image/0.25.10/source/src/io/image_reader_type.rs)).
- **JPEG: nothing else is covered.** `JpegDecoder::set_limits` checks the dimensions and stores the limits; when decoding it hands `zune-jpeg` only `max_image_width` and `max_image_height` ([source, `codecs/jpeg/decoder.rs` lines 168 to 174 and 207 to 225](https://docs.rs/crate/image/0.25.10/source/src/codecs/jpeg/decoder.rs)). The decoder's copy of the input (`r.read_to_end(&mut input)`, lines 31 to 33) and `zune-jpeg`'s coefficient store are not counted.
- **PNG: the `png` crate's own allocations are bounded too.** `PngDecoder::with_limits` passes `max_alloc` on as `png::Limits { bytes }` ([source, `codecs/png.rs` lines 46 to 52](https://docs.rs/crate/image/0.25.10/source/src/codecs/png.rs)). `ImageReader` passes the whole `max_alloc` there, not what is left after the output buffer.
- **WebP: nothing else is covered.** `image`'s `WebPDecoder` does not override `set_limits`, so it gets the trait's default, a dimension check ([source](https://docs.rs/crate/image/0.25.10/source/src/codecs/webp/decoder.rs)). `image-webp` has a `set_memory_limit` that `image` never calls; it defaults to `usize::MAX` and is used only when reading metadata chunks ([source, `decoder.rs` lines 354, 554 and 643 to 654](https://docs.rs/crate/image-webp/0.2.4/source/src/decoder.rs)).
- Checked by running it. Peak RSS is the whole process, about 3 MiB of it the idle binary:

  | File | Limit set | Result | Peak RSS, MiB |
  | --- | --- | --- | --- |
  | 4K JPEG, baseline | `max_alloc` 20 MiB, under the 23.7 MiB bitmap | "Memory limit exceeded" | 3.6 |
  | 4K JPEG, baseline | `max_alloc` 24 MiB | decodes | 27.9 |
  | 4K JPEG, progressive | `max_alloc` 24 MiB | decodes | 51.4 |
  | 4K RGB PNG | `max_alloc` 24 MiB | decodes | 25.9 |
  | 4000x4000 RGBA PNG | `max_alloc` 60 MiB, under the 61.0 MiB bitmap | "Memory limit exceeded" | 2.7 |
  | 4000x4000 RGBA PNG | `max_alloc` 62 MiB | decodes | 62.9 |
  | 4K lossy WebP | `max_alloc` 20 MiB | "Memory limit exceeded" | 2.6 |
  | 4K lossy WebP | `max_alloc` 24 MiB | decodes | 38.7 |
  | 4K JPEG, 4000x4000 PNG, 4K WebP | `max_image_width` or `max_image_height` below the image | "Image size exceeds limit" | 2.7 to 3.7 |

### c. What `fast_image_resize` needs and allocates

- **It needs the whole source in memory.** A source is an `ImageView`, whose one required accessor is `iter_rows(start_row)`, an iterator of row slices ([source, `image_view.rs`](https://docs.rs/crate/fast_image_resize/6.1.0/source/src/image_view.rs)). For a `DynamicImage` the view is built over `as_bytes()`, the one contiguous buffer ([source, `images/image_crate.rs`](https://docs.rs/crate/fast_image_resize/6.1.0/source/src/images/image_crate.rs)).
- **It cannot resize in place.** `Resizer::resize(&mut self, src_image: &impl IntoImageView, dst_image: &mut impl IntoImageViewMut, options)` takes two separate images ([docs](https://docs.rs/fast_image_resize/6.1.0/fast_image_resize/struct.Resizer.html)).
- **What it allocates** ([source, `resizer.rs`](https://docs.rs/crate/fast_image_resize/6.1.0/source/src/resizer.rs)):
  - An intermediate image for the two-pass convolution. For 8-bit pixels the vertical pass runs first, so the intermediate is source width x destination height pixels (lines 410 to 444). Measured: 4.0 MiB for 4K to 640 px, 11.9 MiB for 4K to 1920 px.
  - A premultiplied copy of the whole source when `ResizeOptions::mul_div_alpha` is true (the default) and the pixel type has alpha (lines 322 to 352). `MulDiv::is_supported` lists `U8x2`, `U8x4`, `U16x2`, `U16x4`, `F32x2`, `F32x4` ([source, `mul_div.rs`](https://docs.rs/crate/fast_image_resize/6.1.0/source/src/mul_div.rs)).
  - Coefficient tables, which are small.
- **The `Resizer` keeps those buffers** between calls, as `alpha_buffer`, `convolution_buffer` and `super_sampling_buffer` (lines 137 to 143). `size_of_internal_buffers()` reports them and `reset_internal_buffers()` frees them (lines 278 to 299). A `Resizer` kept alive in the Server would hold the last job's 4 to 90 MiB; create one per job or reset it.
- **Strips.** There is no streaming or strip API. `ResizeOptions::crop(left, top, width, height)` resizes a region of a source that is in memory, and with the `rayon` feature the crate splits the image by height internally; neither removes the need for the whole source. Resizing a source strip by strip, with rows fed from a decoder, would have to be built by hand on top of `crop` and was not tried.
- **Avoiding the alpha copy**: `MulDiv::multiply_alpha_inplace` and `divide_alpha_inplace` ([docs](https://docs.rs/fast_image_resize/6.1.0/fast_image_resize/struct.MulDiv.html)):

  ```rust
  let mul_div = fast_image_resize::MulDiv::new();
  mul_div.multiply_alpha_inplace(&mut src)?;                   // src is not used again
  resizer.resize(&src, &mut dst, &options.use_alpha(false))?;
  mul_div.divide_alpha_inplace(&mut dst)?;
  ```

### d. How freed buffers return to the kernel

Rust's default allocator on Unix is the C library's: `System` "is based on malloc on Unix platforms and HeapAlloc on Windows" ([std docs](https://doc.rust-lang.org/std/alloc/struct.System.html)), and this workspace sets no other global allocator. In glibc, a request at or above `M_MMAP_THRESHOLD` is given its own `mmap`, so that "the memory for these chunks can be returned to the system on free"; the threshold's default is 128 KiB, but "the threshold is adjusted dynamically to suit the allocation patterns of the program" unless it is set ([glibc manual](https://sourceware.org/glibc/manual/latest/html_node/Malloc-Tunable-Parameters.html)). The rule is in mallopt(3): "when blocks larger than the current threshold and less than or equal to DEFAULT_MMAP_THRESHOLD_MAX are freed, the threshold is adjusted upward to the size of the freed block", the trim threshold becomes "twice the dynamic mmap threshold", the maximum is "4\*1024\*1024\*sizeof(long)" on 64-bit systems (32 MiB) and 512 KiB on 32-bit, and the adjustment "is disabled if any of the M_TRIM_THRESHOLD, M_TOP_PAD, M_MMAP_THRESHOLD, or M_MMAP_MAX parameters is set" ([man page](https://man7.org/linux/man-pages/man3/mallopt.3.html)). So the first `free` of a 23.7 MiB bitmap moves the threshold to 23.7 MiB, later bitmaps are cut from a heap, and a heap gives memory back only when its free top exceeds the trim threshold, now 47.5 MiB; that is the 30 MiB the measurements show being kept per thread. The same code is in glibc 2.41, the version in Debian trixie, which the Docker image is built on ([malloc.c lines 3453 to 3465](https://github.com/bminor/glibc/blob/glibc-2.41/malloc/malloc.c#L3453-L3465), a mirror of sourceware's repository). `malloc_trim` "frees memory in all arenas and in all chunks with whole free pages" by its man page ([malloc_trim(3)](https://man7.org/linux/man-pages/man3/malloc_trim.3.html)), but in the source only the main arena has its top trimmed ([malloc.c lines 5206 to 5255](https://github.com/bminor/glibc/blob/glibc-2.41/malloc/malloc.c#L5206-L5255)), which matches the 1 MiB it returned on a spawned thread. Setting the threshold through `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=N`, which "supersedes the MALLOC_MMAP_THRESHOLD_ environment variable" ([glibc manual](https://sourceware.org/glibc/manual/latest/html_node/Memory-Allocation-Tunables.html)), or through `mallopt`, turns the adjustment off. musl differs: its allocator has a fixed `MMAP_THRESHOLD` of 131052 bytes, maps each larger request and unmaps it on `free` ([meta.h](https://git.musl-libc.org/cgit/musl/tree/src/malloc/mallocng/meta.h?h=v1.2.5), [malloc.c](https://git.musl-libc.org/cgit/musl/tree/src/malloc/mallocng/malloc.c?h=v1.2.5#n308), [free.c](https://git.musl-libc.org/cgit/musl/tree/src/malloc/mallocng/free.c?h=v1.2.5#n143)), so it has no such effect; but this repo ships no musl build. On 32-bit glibc the 512 KiB maximum means image-sized buffers always use `mmap`. macOS and Windows were not checked.

Why threads matter for the Server: tokio runs `spawn_blocking` work on a pool of up to 512 threads that "will exit if left idle for too long", by default after 10 seconds ([tokio 1.54.1 docs](https://docs.rs/tokio/1.54.1/tokio/runtime/struct.Builder.html#method.max_blocking_threads)). A semaphore limits how many image jobs run at once, not which pool threads run them.

### e. PNG and WebP decoding in `image`

- **Whole image only, for every format.** `ImageDecoder::read_image(self, buf: &mut [u8])` fills one buffer of `total_bytes()`, which `image` allocates in one piece (`vec![zero; total_bytes]`, [source, `io/free_functions.rs` lines 305 to 319](https://docs.rs/crate/image/0.25.10/source/src/io/free_functions.rs)). There is no reduced-size or region decode for PNG or WebP.
- **PNG.** `image` calls `png::Reader::next_frame(buf)` for the whole frame ([source, `codecs/png.rs` lines 241 to 244](https://docs.rs/crate/image/0.25.10/source/src/codecs/png.rs)). Beside the bitmap the decoder held under 1 MiB (measured: 0.7 MiB for both PNGs). The `png` crate itself can hand out one row at a time (`Reader::next_row`, `read_row`, [docs](https://docs.rs/png/0.18.1/png/struct.Reader.html)); `image` does not expose that. A 16-bit PNG decodes to 16-bit samples, twice the bytes; none was measured.
- **Lossy WebP.** `image-webp` decodes the whole frame into three planes, `ybuf`, `ubuf` and `vbuf`, then converts into the output ([source, `vp8.rs` lines 770 to 784](https://docs.rs/crate/image-webp/0.2.4/source/src/vp8.rs), [`decoder.rs` lines 669 to 740](https://docs.rs/crate/image-webp/0.2.4/source/src/decoder.rs)). Measured: 13.1 MiB beside the 23.7 MiB bitmap for the 4K file, and 140 ms, six times the JPEG.
- **Lossless WebP.** With alpha it decodes straight into the output (measured: 2.8 MiB beside the bitmap). Without alpha it first decodes into a temporary 4-byte-per-pixel buffer and copies three bytes of each pixel out (`decoder.rs` line 687), so the peak is 7 bytes per pixel; that case was not measured.
- **Orientation.** `DynamicImage::apply_orientation` rotates by 90 or 270 degrees into a new image while the old one is alive ([source, `images/dynimage.rs` lines 1161 to 1181](https://docs.rs/crate/image/0.25.10/source/src/images/dynimage.rs)), a second full bitmap if done before the resize. Applying it to the resized image avoids that. Not measured.
- **JPEG, for comparison.** `zune-jpeg` decodes a baseline interleaved file one row of blocks at a time into the output ([source, `mcu.rs` lines 159 to 164](https://docs.rs/crate/zune-jpeg/0.5.15/source/src/mcu.rs)). For a progressive file it allocates the coefficients of the whole image, 16 bits each, for every component ([source, `mcu_prog.rs` lines 101 to 106](https://docs.rs/crate/zune-jpeg/0.5.15/source/src/mcu_prog.rs)); a baseline file whose components come in separate scans gets the same treatment (`mcu.rs` lines 173 to 184).

Bytes held per source pixel at the decode peak, from the above; the compressed file comes on top for JPEG:

| Source | Bitmap | Beside it | Total | Measured |
| --- | --- | --- | --- | --- |
| JPEG, baseline, interleaved | 3 | about 0 | 3 | yes |
| JPEG, progressive, 4:2:0 | 3 | 3 | 6 | yes |
| JPEG, progressive, 4:4:4 | 3 | 6 | 9 | no, from the source |
| PNG, 8-bit RGB or RGBA | 3 or 4 | about 0 | 3 or 4 | yes |
| WebP, lossy, no alpha | 3 | 1.5 | 4.5 | yes |
| WebP, lossless, with alpha | 4 | about 0 | 4 | yes |
| WebP, lossless, no alpha | 3 | 4 | 7 | no, from the source |

The resize then holds the bitmap, source width x destination height x channels, and the output; the encoder holds the output plus about 6 to 8 bytes per output pixel for RGB WebP, 27 to 32 for RGBA WebP, and next to nothing for JPEG.

### f. What Next.js, sharp, libvips and Jellyfin do

- **libvips does not hold the image.** "Because the intermediate image is just a small region in memory, a pipeline of operations running together needs very little RAM" ([How libvips works](https://www.libvips.org/API/current/how-it-works.html)). `vips_thumbnail` shrinks "in three stages: using any shrink-on-load features available in the image load library, using a block shrink, and using a lanczos3 shrink" ([docs](https://www.libvips.org/API/current/ctor.Image.thumbnail.html)), and its JPEG loader takes `shrink`, "Possible values are 1, 2, 4 and 8. Shrinking during read is very much faster than decompressing the whole image and then shrinking later" ([docs](https://www.libvips.org/API/current/ctor.Image.jpegload.html)). That is the scaled decoding of section a.
- **sharp limits threads, not jobs, and blames glibc malloc.** `sharp.concurrency()` is "the maximum number of threads libvips should use to process each image"; "The default value is the number of CPU cores, except when using glibc-based Linux without jemalloc, where the default is 1 to help reduce memory fragmentation" ([docs](https://sharp.pixelplumbing.com/api-utility#concurrency)). Its install page says "The default memory allocator on most glibc-based Linux systems (e.g. Debian, Red Hat) is unsuitable for long-running, multi-threaded processes that involve lots of small memory allocations" and recommends jemalloc ([docs](https://sharp.pixelplumbing.com/install#linux-memory-allocator)); its performance page suggests `MALLOC_ARENA_MAX="2"` ([docs](https://sharp.pixelplumbing.com/performance#parallelism-and-concurrency)). How many images run at once is libuv's thread pool, `UV_THREADPOOL_SIZE`, "which defaults to 4" (same page). sharp also has `limitInputPixels`, default 268,402,689, and `sequentialRead`, default true ([docs](https://sharp.pixelplumbing.com/api-constructor)), and a libvips operation cache capped at 50 MB, 20 files and 100 operations ([docs](https://sharp.pixelplumbing.com/api-utility#cache)).
- **Next.js halves sharp's thread count.** `getSharp` sets `sharp.concurrency()` to half its default (a quarter in development) under the comment "Reducing concurrency should reduce the memory usage too" ([transform.ts lines 88 to 130](https://github.com/vercel/next.js/blob/v16.4.6/packages/next/src/server/image-optimizer/transform.ts#L88-L130)). It passes `limitInputPixels` and `sequentialRead` and a 7 second timeout (lines 157 to 164); the defaults are `imgOptMaxInputPixels: 268_402_689` and `imgOptTimeoutInSeconds: 7` ([config-shared.ts lines 2428 to 2433](https://github.com/vercel/next.js/blob/v16.4.6/packages/next/src/server/config-shared.ts#L2428-L2433)). No limit on how many images are optimized at once was found in the optimizer's source.
- **Jellyfin limits jobs with a semaphore sized to the CPU count.** `ParallelImageEncodingLimit` is a server setting, "the limit for parallel image encoding" ([ServerConfiguration.cs lines 270 to 274](https://github.com/jellyfin/jellyfin/blob/v12.4/MediaBrowser.Model/Configuration/ServerConfiguration.cs#L270-L274)). Below 1, which is the default, the semaphore gets `Environment.ProcessorCount` permits ([ImageProcessor.cs lines 68 to 74](https://github.com/jellyfin/jellyfin/blob/v12.4/src/Jellyfin.Drawing/ImageProcessor.cs#L68-L74)), and the encode runs under it with the comment "Limit number of parallel (more precisely: concurrent) image encodings to prevent a high memory usage" ([lines 201 to 205](https://github.com/jellyfin/jellyfin/blob/v12.4/src/Jellyfin.Drawing/ImageProcessor.cs#L201-L205)). Its Skia decode path creates a bitmap of the codec's full width and height ([SkiaEncoder.cs lines 368 to 387](https://github.com/jellyfin/jellyfin/blob/v12.4/src/Jellyfin.Drawing.Skia/SkiaEncoder.cs#L368-L387)), so it too holds the whole image.

## Where this differs from the earlier survey

- The survey says of `Limits` that "only the PNG, JPEG, TIFF and GIF decoders override `set_limits` to count their internal allocations". For JPEG in 0.25.10 that is not so: the override stores the limits and applies only the width and height (section b, and the 51.4 MiB run under a 24 MiB `max_alloc`). The survey's advice stands and needs one addition: beside explicit width and height, the Server has to budget JPEG and WebP decoding itself.
- The survey has no entry for `turbojpeg`. It passes this repo's cargo-deny as declared, but it is a C library with a CMake and NASM build and an IJG-licensed core (section a).
- The survey's recommendation to resize with `fast_image_resize` holds, with one caveat it does not mention: the default options double the memory of an alpha source (section c).
- The survey suggests running the work "under `tokio::task::spawn_blocking` behind a semaphore". Measured here: with glibc's default settings that keeps about 30 MiB per pool thread that has ever run a 4K job (section d).

## Not verified

- Nothing was run on a low-memory device, on ARM, or on a 32-bit system. Times on such a CPU will be several times longer; `fast_image_resize`'s and `zune-jpeg`'s NEON paths were not exercised.
- The test images are synthetic. Decode time depends on the content; memory does not, beyond the file size.
- Whether metadata providers' posters and backdrops are progressive JPEGs. It decides whether the common job costs 26 to 30 MiB or 36 to 50; a file with an SOF2 marker (`ff c2`) is progressive.
- Progressive 4:4:4 JPEG, 16-bit PNG, CMYK JPEG, animated GIF and WebP, and lossless WebP without alpha were not measured; where a figure is given it is computed from the source.
- jellyfin-ffmpeg, the ffmpeg in the Docker image, was not run. It is linked differently from the Arch build measured here, so its 43.8 MiB start-up cost and its file-backed share will differ.
- glibc 2.41 (Debian trixie) was read, not run; the measurements are on glibc 2.44. musl, macOS and Windows allocators were not measured.
- What a fixed mmap threshold costs the rest of the Server (SQLite, the plugin runtime, HTTP buffers) was not measured, only the image job. Values other than 131072 were not tried.
- Whether tokio hands consecutive blocking jobs to the same pool thread. The four-thread test shows what happens if it does not.
- Resizing in strips through `ResizeOptions::crop` was not tried, nor row-by-row PNG decoding through the `png` crate.
- `fast_image_resize`'s `rayon` feature and `image`'s `rayon` feature were off. With them on, more threads and more arenas are in play.
- libwebp 1.6.0 (through `libwebp-sys` 0.14) was not measured; the WebP encoder figures are for the tree `libwebp-sys` 0.9.6 vendors, at `method` 4 unless stated.
- `jpeg-decoder`'s handling of EXIF orientation, ICC profiles and CMYK files, which the Server would have to do itself on that path. Its output was compared with `image`'s only for one file (41 dB PSNR after WebP encoding).
- JPEG XL: only libjxl at effort 7 and distance 1.0. Lower efforts and any Rust encoder were not measured.
- The high-water marks read up to about 1 MiB low (see Method). The wall times were taken on a busy machine.
- The cargo-deny runs covered `licenses`, `advisories` and `sources`, as in the earlier survey; `bans` was not run.
