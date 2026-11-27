import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The disk the subtitle cache takes now, in bytes, as the Playback settings were last read. */
export function useSubtitleCacheBytes() {
  return useSuspenseQuery(
    ServerService.method.getPlaybackSettings,
    {},
    { staleTime: loaderStaleTime },
  ).data.subtitleCacheBytes;
}
