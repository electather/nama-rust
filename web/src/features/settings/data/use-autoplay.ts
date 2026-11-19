import { useQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Whether the next Episode plays by itself for the signed-in User. */
export function useAutoplay() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- a switch in the Playback section, which shows nothing for it until it loads
  return useQuery(PlaybackService.method.getAutoplay, {});
}
