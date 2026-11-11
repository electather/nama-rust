# Images are resized inside the Server, one at a time, and sent as JPEG XL or JPEG

The Server keeps each image at full size (ADR 0037) and, while an admin leaves resizing on, sends a smaller copy made on request. It resizes only to widths on a fixed list, at one quality, never larger than the original, and keeps the copies in a cache with a size limit that drops the least recently used. The work runs inside the Server on `image` and `fast_image_resize`, one resize at a time on a single thread, within 128 MiB above the Server's idle memory, because the Server has to run well on a machine with 1 GiB of RAM. A browser that asks for JPEG XL gets it, and every other client gets JPEG.

## Considered Options

- **A child ffmpeg for each resize**: rejected. The Server already uses ffmpeg for Transcodes, but it took 60 to 68 MiB and longer for a job that takes 26 to 51 MiB inside the Server, and 44 MiB of that is ffmpeg starting (`docs/research/image-memory-use.md`). jellyfin-ffmpeg has no JPEG XL encoder, and the Server may run without ffmpeg (ADR 0026).
- **WebP**, which Jellyfin and Next.js send: rejected. Lossy WebP needs libwebp, which is C. With JPEG XL going to Chrome, Firefox and Safari, WebP would serve only Edge, Samsung Internet and TV browsers, which JPEG serves without any C code (`docs/research/jpeg-xl-as-served-format.md`).
- **AVIF**: rejected. Both encoders fail `deny.toml` today (`docs/research/rust-crates-for-stored-images.md`).
- **Any size a client asks for**, as Jellyfin accepts: rejected. Before Jellyfin 12.0 one request could pin a core and gigabytes of memory, and its cache has no size limit (`docs/research/jellyfin-and-plex-images.md`).
- **An image proxy beside the Server**, such as imgproxy: rejected. It is a second process to ship, start and secure.
- **Scaled JPEG decoding through `jpeg-decoder`**: left out for now. It cut a 4K-to-640 px job from 30 MiB to 5 MiB, but it is a second JPEG decoder beside `image`'s, and its README says it is in maintenance mode. It is the first thing to reach for if memory proves short.

## Consequences

- An admin can turn resizing off. The Server then sends the stored image and the browser scales it.
- The JPEG XL encoder isn't chosen yet. A prototype compares `jxl-encoder` and `jixel`: whether the three browsers show their files, how long an encode takes, and its peak memory at a low effort. libjxl is the fallback, and it needed 70 to 86 bytes per output pixel at its default effort.
- A width whose JPEG XL encode doesn't fit the ceiling is sent as JPEG.
- An image over 12 megapixels isn't resized. A Custom Image over it is refused at upload, and a downloaded one is sent as it is. The Server checks the pixel count itself before decoding, because `image`'s `max_alloc` doesn't bound a decoder's own buffers.
- glibc keeps about 31 MiB for each thread that has run a resize, which is why the work has one thread. The Docker image sets `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=131072`, which hands that memory back.
- JPEG XL files are written progressively, so Chrome and Firefox paint a blur from the first kilobytes. That costs 4 to 9% more bytes. Safari paints only finished files.
- Everywhere else the placeholder is a small hash sent with the image's URL. It is computed during an image's first resize, when the image is already decoded, so there are no placeholders with resizing off.
- A width that both kinds of client ask for is kept twice in the cache, and responses vary by `Accept`.
- An SVG logo is never resized.
