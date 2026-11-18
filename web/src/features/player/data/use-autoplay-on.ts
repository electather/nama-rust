import { useQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Whether the User has autoplay on, which it isn't until the Server says. */
export function useAutoplayOn() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Episode plays while the Server answers
  return useQuery(PlaybackService.method.getAutoplay, {}).data?.on === true;
}
