import { useSuspenseQuery } from "@connectrpc/connect-query";

import { ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** What ffmpeg and ffprobe the Server found, and the hardware acceleration that passed its test. */
export function usePlaybackStatus() {
  return useSuspenseQuery(
    ServerService.method.getPlaybackStatus,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
