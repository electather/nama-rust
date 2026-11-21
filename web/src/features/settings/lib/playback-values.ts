import {
  type GetSubtitleAppearanceResponse,
  type GetTrackPreferencesResponse,
  SubtitleKind,
  type UsersSubtitlePreference,
} from "@/gen/nama/v1/playback_pb";

/** The Playback pane's settings, as its controls hold them. */
export interface PlaybackValues {
  /** A language's code, or `""` for the file's default. */
  audio: string;
  /** A language's code, or `""` for subtitles off. */
  subtitles: string;
  which: "full" | "forced";
  sdh: boolean;
  onlyOther: boolean;
  /** A `SubtitleSize`, as its choice is valued. */
  size: string;
  /** A `SubtitleBackground`, as its choice is valued. */
  background: string;
  autoplay: boolean;
}

/** Each setting's name, as its row says it, and its pane's toast when a change to it is refused. */
export const playbackLabels: Record<keyof PlaybackValues, string> = {
  audio: "Audio language",
  subtitles: "Subtitle language",
  which: "Which subtitles",
  sdh: "Include sounds (SDH)",
  onlyOther: "Only for audio in another language",
  size: "Subtitle size",
  background: "Subtitle background",
  autoplay: "Play the next Episode automatically",
};

/** The pane's settings as the Server has them saved. */
export function savedPlayback(saved: {
  tracks: GetTrackPreferencesResponse;
  appearance: GetSubtitleAppearanceResponse;
  autoplay: boolean;
}): PlaybackValues {
  return {
    audio: saved.tracks.audio?.language ?? "",
    ...savedSubtitle(saved.tracks.subtitle),
    size: String(saved.appearance.size),
    background: String(saved.appearance.background),
    autoplay: saved.autoplay,
  };
}

// What Full subtitles start with, as the pane first offers them.
const offered = { sdh: false, onlyOther: true };

/** The Subtitle Preference's settings: off, with the rest as first offered, without one. */
function savedSubtitle(subtitle: UsersSubtitlePreference | undefined) {
  if (subtitle?.language === undefined) {
    return { subtitles: "", which: "full" as const, ...offered };
  }
  if (subtitle.kind === SubtitleKind.FORCED) {
    // Forced-only subtitles hold neither, and each change is saved, so going back to Full starts
    // as first offered.
    return { subtitles: subtitle.language, which: "forced" as const, ...offered };
  }
  return {
    subtitles: subtitle.language,
    which: "full" as const,
    sdh: subtitle.kind === SubtitleKind.SDH,
    onlyOther: subtitle.foreignAudioOnly,
  };
}

/** The settings that make up the Subtitle Preference, which is saved whole. */
export const subtitleKeys = ["subtitles", "which", "sdh", "onlyOther"] as const;

/** The Subtitle Preference `values` ask for: off without a language. */
export function wantedSubtitle(values: PlaybackValues) {
  if (values.subtitles === "") {
    return {};
  }
  const forced = values.which === "forced";
  return {
    language: values.subtitles,
    kind: kindOf(forced, values.sdh),
    // The Server refuses forced-only subtitles only over audio in another language.
    foreignAudioOnly: !forced && values.onlyOther,
  };
}

function kindOf(forced: boolean, sdh: boolean) {
  if (forced) {
    return SubtitleKind.FORCED;
  }
  return sdh ? SubtitleKind.SDH : SubtitleKind.FULL;
}
