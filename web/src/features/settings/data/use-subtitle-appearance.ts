import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Subtitle Appearance, to refetch after saving part of it. */
export function subtitleAppearanceKey(transport: Transport) {
  return createConnectQueryKey({
    schema: PlaybackService.method.getSubtitleAppearance,
    cardinality: "finite",
    transport,
  });
}

/** The signed-in User's Subtitle Appearance. */
export function useSubtitleAppearance() {
  return useSuspenseQuery(
    PlaybackService.method.getSubtitleAppearance,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
