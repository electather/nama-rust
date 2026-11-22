import { create } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PlaybackSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The saved Playback settings, or the defaults while none are saved. */
export function usePlaybackSettings() {
  return (
    useSuspenseQuery(ServerService.method.getPlaybackSettings, {}, { staleTime: loaderStaleTime })
      .data.settings ?? create(PlaybackSettingsSchema)
  );
}
