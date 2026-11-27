import { useState } from "react";

import type { Trailer } from "@/gen/nama/v1/request_pb";

/** `trailer`'s thumbnail, or `art` without one or once it fails to load, as `onError` hears. */
export function useThumbnailOr(
  trailer: Trailer,
  art: { url: string | undefined; thumbhash: Uint8Array | undefined },
) {
  // The Server serves only a thumbnail it stored, so showing it tells the Trailer's host nothing;
  // one not stored yet isn't found. Kept by URL, so a new thumbnail in its place gets its own try.
  const [failedUrl, setFailedUrl] = useState<string>();
  const shown =
    trailer.thumbnailUrl === undefined || trailer.thumbnailUrl === failedUrl
      ? art
      : { url: trailer.thumbnailUrl, thumbhash: trailer.thumbhash };
  return {
    shown,
    onError: () => {
      setFailedUrl(trailer.thumbnailUrl);
    },
  };
}
