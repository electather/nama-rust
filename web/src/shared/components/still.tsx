import { Thumbhash } from "@/shared/components/thumbhash";
import { useSrcSet } from "@/shared/hooks/use-src-set";
import { backdropGridSizes } from "@/shared/lib/backdrop-grid-sizes";

/**
 * An Episode's Still in a column of `BackdropGrid`, its thumbhash until it loads, or without one
 * a plain box of the same size. A `span`, so it can sit inside a link.
 */
export function Still({
  url,
  thumbhash,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
}) {
  const srcSet = useSrcSet(url);
  return (
    <span className="relative isolate block aspect-video overflow-hidden rounded-lg border bg-muted">
      <Thumbhash thumbhash={thumbhash} />
      {url !== undefined && (
        // Its title is under it, so the image itself says nothing more.
        <img
          src={url}
          srcSet={srcSet}
          sizes={backdropGridSizes}
          alt=""
          loading="lazy"
          className="size-full object-cover"
        />
      )}
    </span>
  );
}
