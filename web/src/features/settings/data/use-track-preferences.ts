import { createConnectQueryKey, useSuspenseQuery } from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The query key of the Track Preferences, to refetch after saving one. */
export const trackPreferencesKey = createConnectQueryKey({
  schema: PlaybackService.method.getTrackPreferences,
  cardinality: "finite",
});

/** The signed-in User's own Track Preferences. */
export function useTrackPreferences() {
  return useSuspenseQuery(
    PlaybackService.method.getTrackPreferences,
    {},
    { staleTime: loaderStaleTime },
  ).data;
}
