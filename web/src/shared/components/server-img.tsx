import { type ReactElement, useState } from "react";

import { ShownServerImg, type ServerImgLook } from "@/shared/components/shown-server-img";
import type { ServerImage } from "@/shared/lib/server-image";

/**
 * The first of `images`, an image and then its stand-ins, that hasn't failed to load, on its own
 * Server at its kind's widths, over its thumbhash; or `fallback` when none is left. Its box must be
 * `relative` and `isolate` for the thumbhash to sit behind it.
 */
export function ServerImg({
  images,
  fallback = null,
  ...look
}: ServerImgLook & {
  images: readonly (ServerImage | undefined)[];
  fallback?: ReactElement | string | null;
}) {
  // Kept by URL, so a new image in a failed one's place gets its own try.
  const [failed, setFailed] = useState<readonly string[]>([]);
  const shown = images.find((image) => image !== undefined && !failed.includes(image.url));
  if (shown === undefined) {
    return fallback;
  }
  return (
    <ShownServerImg
      image={shown}
      look={look}
      onError={() => {
        setFailed([...failed, shown.url]);
      }}
    />
  );
}
