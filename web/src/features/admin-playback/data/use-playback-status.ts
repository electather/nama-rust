import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** What ffmpeg and ffprobe the Server found, and the hardware acceleration that passed its test. */
export function playbackStatusQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getPlaybackStatus, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The Playback status `playbackStatusQuery` loads. */
export function usePlaybackStatus() {
  return useSuspenseQuery(
    ServerService.method.getPlaybackStatus,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
