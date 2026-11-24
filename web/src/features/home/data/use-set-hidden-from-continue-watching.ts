import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";

/** Each `useSetHiddenFromContinueWatching` mutation's, for Home to drop a card before it's done. */
export const setHiddenKey = [WatchStateService.method.setHiddenFromContinueWatching.name];

/**
 * Hides a Movie or Show from your Continue Watching, or shows it again, then refetches what's on
 * screen.
 */
export function useSetHiddenFromContinueWatching() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const { mutate } = useMutation(WatchStateService.method.setHiddenFromContinueWatching, {
    mutationKey: setHiddenKey,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey(transport) });
    },
  });
  return mutate;
}
