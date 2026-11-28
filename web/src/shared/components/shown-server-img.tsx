import { useState } from "react";

import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerInfo } from "@/shared/hooks/use-server-info";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { type ServerImage, srcSetOf, widthsOf } from "@/shared/lib/server-image";

/** How a `ServerImg` shows: the image's classes, how wide it shows, what it says and when it loads. */
export interface ServerImgLook {
  /** How wide it shows, for its `srcset`. */
  sizes: string;
  /** The `<img>`'s own classes; its box is the caller's. */
  className: string;
  /** What it shows, for assistive technology: nothing unless given, as its shell names it. */
  alt?: string;
  /** Whether it's the page's largest paint, so it loads first rather than once near the view. */
  largest?: boolean;
  /** Whether it fades in over its thumbhash once loaded, rather than popping in. */
  fades?: boolean;
}

/** A full image before it loads, over its thumbhash; a crossfade under reduced motion. */
const hiddenArt = "scale-103 opacity-0 motion-reduce:scale-100";
/** How it comes in: it settles from slightly large as it fades. */
const fadingArt = "transition-settle motion-reduce:duration-200";

/**
 * `image` over its thumbhash, as `ServerImg` shows it. Apart from `ServerImg`, so only an image
 * shown reads the Server's widths: an error page may show because the Server can't be reached.
 */
export function ShownServerImg({
  image,
  look: { sizes, className, alt = "", largest = false, fades = false },
  onError,
}: {
  image: ServerImage;
  look: ServerImgLook;
  onError: () => void;
}) {
  const src = useServerUrl(image.url);
  const srcSet = srcSetOf(src, widthsOf(image.kind, useServerInfo()));
  // Kept by URL, so a new image in its place fades in too.
  const [loaded, setLoaded] = useState<string>();
  const hidden = fades && loaded !== src;
  return (
    <>
      <Thumbhash thumbhash={image.thumbhash} />
      <img
        src={src}
        srcSet={srcSet}
        sizes={sizes}
        alt={alt}
        loading={largest ? undefined : "lazy"}
        fetchPriority={largest ? "high" : undefined}
        className={fades ? `${className} ${hidden ? hiddenArt : ""} ${fadingArt}` : className}
        ref={(img) => {
          // One already in the cache has loaded before React listens.
          if (img?.complete === true && img.naturalWidth > 0) {
            setLoaded(src);
          }
        }}
        onLoad={() => {
          setLoaded(src);
        }}
        onError={onError}
      />
    </>
  );
}
