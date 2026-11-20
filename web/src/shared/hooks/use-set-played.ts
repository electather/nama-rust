import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";

/** Each `useSetPlayed` mutation's, for a screen to show what's being marked before it's done. */
export const setPlayedKey = [WatchStateService.method.setPlayed.name];

/**
 * Marks a Movie, Episode, Season or Show played or unplayed, then refetches what's on screen so
 * every poster, Episode and Continue Watching shows it.
 */
export function useSetPlayed() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.setPlayed, {
    mutationKey: setPlayedKey,
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey });
    },
  });
  return mutate;
}
