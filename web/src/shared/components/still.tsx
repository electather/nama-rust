import type { ReactNode } from "react";

import { ServerImg } from "@/shared/components/server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * An Episode's Still, `sizes` wide, its thumbhash until it loads, or without one a plain box of the
 * same size, with `children` over it. A `span`, so it can sit inside a link.
 */
export function Still({
  still,
  sizes,
  children,
}: {
  still: ServerImage | undefined;
  sizes: string;
  children?: ReactNode;
}) {
  return (
    <span className="relative isolate block aspect-video overflow-hidden rounded-lg border bg-muted">
      {/* Its title is under it, so the image itself says nothing more. */}
      <ServerImg images={[still]} sizes={sizes} className="size-full object-cover" />
      {children}
    </span>
  );
}
