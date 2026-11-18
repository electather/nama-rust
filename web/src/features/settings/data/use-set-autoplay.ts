import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { autoplayKey } from "@/features/settings/data/use-autoplay";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Saves whether the next Episode plays by itself, then refetches it. */
export function useSetAutoplay() {
  const queryClient = useQueryClient();
  return useMutation(PlaybackService.method.setAutoplay, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: autoplayKey });
    },
  });
}
