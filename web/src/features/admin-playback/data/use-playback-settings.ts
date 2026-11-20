import { create } from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { PlaybackSettingsSchema, ServerService } from "@/gen/nama/v1/server_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Playback settings, for the Playback page to load before it shows the form. */
export function playbackSettingsQuery(transport: Transport) {
  return {
    ...createQueryOptions(ServerService.method.getPlaybackSettings, {}, { transport }),
    staleTime: "static" as const,
  };
}

/** The saved Playback settings, or the defaults while none are saved. */
export function usePlaybackSettings() {
  return (
    useSuspenseQuery(ServerService.method.getPlaybackSettings, {}, { staleTime: loaderStaleTime })
      .data.settings ?? create(PlaybackSettingsSchema)
  );
}
