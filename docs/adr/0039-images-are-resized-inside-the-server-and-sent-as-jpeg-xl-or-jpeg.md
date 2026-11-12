# Images are resized inside the Server, one at a time, and sent as JPEG XL or JPEG

The Server keeps each image at full size (ADR 0037) and, while an admin leaves resizing on, sends a smaller copy made on request. It resizes only to widths on a fixed list, at one quality, never larger than the original, and keeps the copies in a cache with a size limit that drops the least recently used. The work runs inside the Server on `image` and `fast_image_resize`, one resize at a time on a single thread, within 128 MiB above the Server's idle memory, because the Server has to run well on a machine with 1 GiB of RAM. A browser that asks for JPEG XL gets it, and every other client gets JPEG.

## Considered Options

- **A child ffmpeg for each resize**: rejected. The Server already uses ffmpeg for Transcodes, but it took 60 to 68 MiB and longer for a job that takes 26 to 51 MiB inside the Server, and 44 MiB of that is ffmpeg starting (`docs/research/image-memory-use.md`). jellyfin-ffmpeg has no JPEG XL encoder, and the Server may run without ffmpeg (ADR 0026).
- **WebP**, which Jellyfin and Next.js send: rejected. Lossy WebP needs libwebp, which is C. With JPEG XL going to Chrome, Firefox and Safari, WebP would serve only Edge, Samsung Internet and TV browsers, which JPEG serves without any C code (`docs/research/jpeg-xl-as-served-format.md`).
- **libjxl**, through `jpegxl-rs`: rejected. It needs two GPL exceptions in `deny.toml` and a C++ build on all five release targets, and it took 70 to 86 bytes per output pixel at its default effort (`docs/research/image-memory-use.md`).
- **`jxl-encoder`**, the other pure-Rust JPEG XL encoder: rejected for now. At effort 3 and distance 1.5 it passed the same checks as `jixel`, but it took 49 to 51 bytes of its own per output pixel to `jixel`'s 14 to 16, so only 2.4 megapixels would fit the ceiling. It is better kept (a company, its output checked against three decoders, no `unsafe`), and it is the replacement if `jixel` stops being maintained.
- **AVIF**: rejected. Both encoders fail `deny.toml` today (`docs/research/rust-crates-for-stored-images.md`).
- **Any size a client asks for**, as Jellyfin accepts: rejected. Before Jellyfin 12.0 one request could pin a core and gigabytes of memory, and its cache has no size limit (`docs/research/jellyfin-and-plex-images.md`).
- **An image proxy beside the Server**, such as imgproxy: rejected. It is a second process to ship, start and secure.
- **Scaled JPEG decoding through `jpeg-decoder`**: left out for now. It cut a 4K-to-640 px job from 30 MiB to 5 MiB, but it is a second JPEG decoder beside `image`'s, and its README says it is in maintenance mode. It is the first thing to reach for if memory proves short.

## Consequences

- An admin can turn resizing off. The Server then sends the stored image and the browser scales it.
- JPEG XL is encoded by `jixel` 0.5.7 at its `UltraFast` speed and distance 1.2, with progressive DC, lossless alpha and one thread. Of the two encoders it used the least memory per output pixel, which #728 ranked first; its files look like the JPEG copies at about half their size, and a 1920x1080 encode takes 63 to 71 ms (`docs/research/jpeg-xl-as-served-format.md`, section 11). Left to its defaults, `jixel` starts a thread per core and codes alpha lossily, which took 251 ms for a 1920x1080 image.
- An output over 7 megapixels is sent as JPEG, or as stored when it has transparency. `jixel` needs about 17 MiB per megapixel, the image it encodes included, and 7 megapixels fit the ceiling. On the width list only a 2:3 image at 2560 px is over it.
- An image over 12 megapixels isn't resized. A Custom Image over it is refused at upload, and a downloaded one is sent as it is. The Server checks the pixel count itself before decoding, because `image`'s `max_alloc` doesn't bound a decoder's own buffers.
- glibc keeps about 31 MiB for each thread that has run a resize, which is why the work has one thread. The Docker image sets `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072`, which hands that memory back.
- Opaque JPEG XL files are written with progressive DC, so Chrome and Firefox paint a blur from the first kilobyte; with `jixel` it costs no bytes. A file with transparency paints nothing early, whichever encoder wrote it, and Safari paints only finished files.
- Everywhere else the placeholder is a small hash sent with the image's URL. It is computed during an image's first resize, when the image is already decoded, so there are no placeholders with resizing off.
- A width that both kinds of client ask for is kept twice in the cache, and responses vary by `Accept`.
- An SVG logo is never resized.
