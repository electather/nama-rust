import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Whether the next Episode plays by itself for the signed-in User. */
export function useAutoplay() {
  return useSuspenseQuery(PlaybackService.method.getAutoplay, {}).data;
}
