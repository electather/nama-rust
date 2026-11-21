import type { ReactNode } from "react";

import { Thumbhash } from "@/shared/components/thumbhash";
import { useSrcSet } from "@/shared/hooks/use-src-set";

/**
 * An Episode's Still, `sizes` wide, its thumbhash until it loads, or without one a plain box of the
 * same size, with `children` over it. A `span`, so it can sit inside a link.
 */
export function Still({
  url,
  thumbhash,
  sizes,
  children,
}: {
  url: string | undefined;
  thumbhash: Uint8Array | undefined;
  sizes: string;
  children?: ReactNode;
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
          sizes={sizes}
          alt=""
          loading="lazy"
          className="size-full object-cover"
        />
      )}
      {children}
    </span>
  );
}
