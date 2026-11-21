import { useSaveAudioPreference } from "@/features/settings/data/use-save-audio-preference";
import { useSaveSubtitlePreference } from "@/features/settings/data/use-save-subtitle-preference";
import { useSetSubtitleBackground } from "@/features/settings/data/use-set-subtitle-background";
import { useSetSubtitleSize } from "@/features/settings/data/use-set-subtitle-size";
import {
  type PlaybackValues,
  subtitleKeys,
  wantedSubtitle,
} from "@/features/settings/lib/playback-values";
import { backgrounds, chosen, sizes } from "@/features/settings/lib/subtitle-appearance";
import { useSetAutoplay } from "@/shared/hooks/use-set-autoplay";

/**
 * Saves the Playback pane: each part of it that changed with its own call, all at once. A part
 * that's refused leaves its settings changed, and the rest saved.
 */
export function useSavePlayback() {
  const audio = useSaveAudioPreference();
  const subtitle = useSaveSubtitlePreference();
  const size = useSetSubtitleSize();
  const background = useSetSubtitleBackground();
  const autoplay = useSetAutoplay();
  return async (values: PlaybackValues, changed: (keyof PlaybackValues)[]) => {
    // Each part: the settings it holds, and the call that saves it.
    const parts: [keys: readonly (keyof PlaybackValues)[], save: () => Promise<unknown>][] = [
      [["audio"], audio.mutateAsync.bind(undefined, { audio: wantedAudio(values.audio) })],
      [subtitleKeys, subtitle.mutateAsync.bind(undefined, { subtitle: wantedSubtitle(values) })],
      [["size"], size.mutateAsync.bind(undefined, { size: chosen(sizes, values.size) })],
      [
        ["background"],
        background.mutateAsync.bind(undefined, {
          background: chosen(backgrounds, values.background),
        }),
      ],
      [["autoplay"], autoplay.mutateAsync.bind(undefined, { on: values.autoplay })],
    ];
    const edited = parts.filter(([keys]) => keys.some((key) => changed.includes(key)));
    await Promise.all(
      edited.map(async ([, save]) => {
        await save();
      }),
    );
  };
}

/** The Audio Preference a language's code asks for: the file's default without one. */
function wantedAudio(language: string) {
  return language === "" ? {} : { language };
}
