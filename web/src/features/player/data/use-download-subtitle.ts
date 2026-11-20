import { useMutation } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Downloads a subtitle a search found into the Server, which adds it to the Version's Tracks. */
export function useDownloadSubtitle() {
  return useMutation(PlaybackService.method.downloadSubtitle);
}
