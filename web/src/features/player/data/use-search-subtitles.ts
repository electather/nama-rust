import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { useMutation } from "@tanstack/react-query";

import { preferredFirst, searchLanguages } from "@/features/player/lib/subtitle-search";
import { PlaybackService } from "@/gen/nama/v1/playback_pb";

/**
 * Searches the subtitle sources for Version `versionId` in the User's Subtitle Preference's
 * language and the browser's, giving what's found in the preferred one first (ADR 0035).
 */
export function useSearchSubtitles(versionId: bigint) {
  const transport = useTransport();
  // A mutation, not a query: it runs only when the User asks, and every source counts the calls.
  return useMutation({
    mutationFn: async () => {
      const preferences = await callUnaryMethod(
        transport,
        PlaybackService.method.getTrackPreferences,
        {},
      );
      const preferred = preferences.subtitle?.language;
      const languages = searchLanguages(preferred, navigator.languages);
      const found = await callUnaryMethod(transport, PlaybackService.method.searchSubtitles, {
        versionId,
        languages,
      });
      return preferredFirst(found.subtitles, preferred);
    },
  });
}
