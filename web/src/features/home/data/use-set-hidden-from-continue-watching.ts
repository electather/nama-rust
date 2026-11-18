import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { WatchStateService } from "@/gen/nama/v1/watch_state_pb";
import { browseKey } from "@/lib/browse";

/**
 * Hides a Movie or Show from your Continue Watching, or shows it again, then refetches what's on
 * screen.
 */
export function useSetHiddenFromContinueWatching() {
  const queryClient = useQueryClient();
  const { mutate } = useMutation(WatchStateService.method.setHiddenFromContinueWatching, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: browseKey });
    },
  });
  return mutate;
}
