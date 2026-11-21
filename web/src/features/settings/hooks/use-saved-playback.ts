import { useAutoplay } from "@/features/settings/data/use-autoplay";
import { useSubtitleAppearance } from "@/features/settings/data/use-subtitle-appearance";
import { useTrackPreferences } from "@/features/settings/data/use-track-preferences";
import { savedPlayback } from "@/features/settings/lib/playback-values";

/** The Playback pane's settings as the Server has them saved. Suspends until they've loaded. */
export function useSavedPlayback() {
  const tracks = useTrackPreferences();
  const appearance = useSubtitleAppearance();
  const autoplay = useAutoplay();
  return savedPlayback({ tracks, appearance, autoplay: autoplay.on });
}
