import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Which of the Server's width lists an image offers: every width it resizes to, or one kind's. */
export type OfferedWidths = "imageWidths" | "posterWidths" | "backdropWidths" | "titleLogoWidths";

/**
 * The `srcset` for the image at `url`: a copy at each of the Server's `offered` widths, so the
 * browser fetches one that fits where it's shown. A kind's list holds only the widths the Server
 * makes ahead, so a first visit waits on no resize. `undefined` without an image, or widths to offer.
 */
export function useSrcSet(url: string | undefined, offered: OfferedWidths = "imageWidths") {
  // Every signed-in page has the Server's info already: its access check fetched it.
  const info = useSuspenseQuery(
    ServerService.method.getServerInfo,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  // A Server older than a kind's list sends none, and still resizes to every width.
  const widths = info[offered].length > 0 ? info[offered] : info.imageWidths;
  return url !== undefined && widths.length > 0
    ? widths.map((width) => `${url}?width=${width} ${width}w`).join(", ")
    : undefined;
}
