import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { trackPreferencesKey } from "@/features/settings/data/use-track-preferences";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Saves the User's own Subtitle Preference, then refetches the Track Preferences. */
export function useSaveSubtitlePreference() {
  const queryClient = useQueryClient();
  return useMutation(PlaybackService.method.setSubtitlePreference, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: trackPreferencesKey });
    },
  });
}
