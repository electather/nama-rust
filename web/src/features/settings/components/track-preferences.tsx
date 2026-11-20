import { AudioForm } from "@/features/settings/components/audio-form";
import { AutoplayPreference } from "@/features/settings/components/autoplay-preference";
import { SubtitleAppearance } from "@/features/settings/components/subtitle-appearance";
import { SubtitlesForm } from "@/features/settings/components/subtitles-form";
import { useTrackPreferences } from "@/features/settings/data/use-track-preferences";

/**
 * The signed-in User's own Track Preferences: the subtitles and audio everything they play starts
 * with, unless they picked others in that Show or Movie (ADR 0035), their Subtitle Appearance, and
 * whether the next Episode plays by itself.
 */
export function TrackPreferences() {
  const data = useTrackPreferences();
  return (
    <section aria-labelledby="playback" className="mt-12 max-w-sm">
      <h2 id="playback" className="font-medium">
        Playback
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        What everything you play starts with, on every Device. Subtitles or audio you pick while
        watching a Show or Movie are kept for that one instead.
      </p>
      <SubtitlesForm saved={data.subtitle} />
      <AudioForm saved={data.audio} />
      <SubtitleAppearance />
      <AutoplayPreference />
    </section>
  );
}
