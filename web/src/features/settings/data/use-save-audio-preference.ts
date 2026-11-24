import { useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { trackPreferencesKey } from "@/features/settings/data/use-track-preferences";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/** Saves the User's own Audio Preference, then refetches the Track Preferences. */
export function useSaveAudioPreference() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(PlaybackService.method.setAudioPreference, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: trackPreferencesKey(transport) });
    },
  });
}
