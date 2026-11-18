import type { Transport } from "@connectrpc/connect";
import {
  createConnectQueryKey,
  createQueryOptions,
  useSuspenseQuery,
} from "@connectrpc/connect-query";

import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** The query key of the Track Preferences, to refetch after saving one. */
export const trackPreferencesKey = createConnectQueryKey({
  schema: PlaybackService.method.getTrackPreferences,
  cardinality: "finite",
});

/** The signed-in User's own Track Preferences, which Settings loads before it shows. */
export function trackPreferencesQuery(transport: Transport) {
  return createQueryOptions(PlaybackService.method.getTrackPreferences, {}, { transport });
}

/** The signed-in User's own Track Preferences. */
export function useTrackPreferences() {
  return useSuspenseQuery(PlaybackService.method.getTrackPreferences, {}).data;
}
