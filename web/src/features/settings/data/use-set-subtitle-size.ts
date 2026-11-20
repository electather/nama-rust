import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { subtitleAppearanceKey } from "@/features/settings/data/use-subtitle-appearance";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Saves the size of the User's text subtitles, then refetches their Subtitle Appearance. */
export function useSetSubtitleSize() {
  const queryClient = useQueryClient();
  return useMutation(PlaybackService.method.setSubtitleSize, {
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: subtitleAppearanceKey });
    },
  });
}
