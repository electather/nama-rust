import type { Transport } from "@connectrpc/connect";
import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";
import { serverKey } from "@/shared/hooks/server-key";

/**
 * Each `useSetHiddenFromContinueWatching` mutation's key on the Server `transport` talks to, for
 * Home to drop a card before it's done.
 */
export const setHiddenKey = (transport: Transport) => [
  WatchStateService.method.setHiddenFromContinueWatching.name,
  serverKey(transport),
];

/**
 * Hides a Movie or Show from your Continue Watching, or shows it again, then refetches what's on
 * screen.
 */
export function useSetHiddenFromContinueWatching() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  const { mutate } = useMutation(WatchStateService.method.setHiddenFromContinueWatching, {
    mutationKey: setHiddenKey(transport),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey(transport) });
    },
  });
  return mutate;
}
