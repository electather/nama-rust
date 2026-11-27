import { Thumbhash } from "@/shared/components/thumbhash";
import { useServerInfo } from "@/shared/hooks/use-server-info";
import { useServerUrl } from "@/shared/hooks/use-server-url";
import { type ServerImage, srcSetOf, widthsOf } from "@/shared/lib/server-image";

/** How a `ServerImg` shows: its box's classes, how wide it shows, and what it says. */
export interface ServerImgLook {
  /** How wide it shows, for its `srcset`. */
  sizes: string;
  className: string;
  /** What it shows, for assistive technology: nothing unless given, as its shell names it. */
  alt?: string;
  /** Whether it's the page's largest paint, so it loads first rather than once near the view. */
  largest?: boolean;
}

/** `image` over its thumbhash, as `ServerImg` shows it. Only this reads the Server's widths. */
export function ShownServerImg({
  image,
  look: { sizes, className, alt = "", largest = false },
  onError,
}: {
  image: ServerImage;
  look: ServerImgLook;
  onError: () => void;
}) {
  const src = useServerUrl(image.url);
  const srcSet = srcSetOf(src, widthsOf(image.kind, useServerInfo()));
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
        className={className}
        onError={onError}
      />
    </>
  );
}
