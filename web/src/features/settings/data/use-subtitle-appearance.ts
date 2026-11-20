import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Subtitle Appearance, to refetch after saving part of it. */
export const subtitleAppearanceKey = createConnectQueryKey({
  schema: PlaybackService.method.getSubtitleAppearance,
  cardinality: "finite",
});

/** The signed-in User's Subtitle Appearance, which Settings loads before it shows. */
export function subtitleAppearanceQuery(transport: Transport) {
  return createQueryOptions(PlaybackService.method.getSubtitleAppearance, {}, { transport });
}

/** The signed-in User's Subtitle Appearance. */
export function useSubtitleAppearance() {
  return useSuspenseQuery(
    PlaybackService.method.getSubtitleAppearance,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
