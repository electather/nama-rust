import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";

/**
 * The `srcset` for the image at `url`: a copy at each width the Server resizes to, so the browser
 * fetches one that fits where it's shown. `undefined` without an image, or widths to offer.
 */
export function useSrcSet(url: string | undefined) {
  // Every signed-in page has the Server's info already: its access check fetched it.
  const { imageWidths } = useSuspenseQuery(ServerService.method.getServerInfo, {}).data;
  const offered = url !== undefined && imageWidths.length > 0;
  return offered
    ? imageWidths.map((width) => `${url}?width=${width} ${width}w`).join(", ")
    : undefined;
}
