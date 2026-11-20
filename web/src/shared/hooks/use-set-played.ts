import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/shared/hooks/browse";

/**
 * Marks a Movie, Episode, Season or Show played or unplayed, then refetches what's on screen so
 * every poster, Episode and Continue Watching shows it.
 */
export function useSetPlayed() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.setPlayed, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey });
    },
  });
  return mutate;
}
