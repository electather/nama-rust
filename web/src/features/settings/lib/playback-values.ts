import {
  type GetSubtitleAppearanceResponse,
  type GetTrackPreferencesResponse,
  SubtitleKind,
  type UsersSubtitlePreference,
} from "@/gen/nama/v1/playback_pb";
import type { SettingText } from "@/shared/lib/setting-text";

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

/**
 * What each setting's row says, in the pane's order. The toast names a refused change by its
 * label. Which subtitles and the switches under it show only while a language is picked.
 */
export const playbackLabels: Record<keyof PlaybackValues, SettingText> = {
  audio: {
    label: "Audio language",
    hint: "Audio starts in this language when the file has it.",
    words: "dub track",
  },
  subtitles: {
    label: "Subtitle language",
    hint: "Pick Off to start with no subtitles.",
    words: "captions cc",
  },
  which: {
    label: "Which subtitles",
    hint: "Full is every line. Forced only is signs and lines in another language.",
    shownBy: "subtitles",
  },
  onlyOther: {
    label: "Only for audio in another language",
    hint: "Subtitles stay off when the audio is already in your subtitle language.",
    words: "foreign",
    shownBy: "subtitles",
  },
  sdh: {
    label: "Include sounds (SDH)",
    hint: "Adds sounds like [door slams] when the file has them.",
    words: "captions hearing deaf cc",
    shownBy: "subtitles",
  },
  size: { label: "Subtitle size", hint: "How big text subtitles are.", words: "text font" },
  background: {
    label: "Subtitle background",
    hint: "Outline is a dark edge around each letter, Box a dark box behind each line. Styled subtitles keep the look they came with.",
    words: "appearance",
  },
  autoplay: {
    label: "Play the next Episode automatically",
    hint: "When an Episode ends, the next one starts after a countdown.",
    words: "autoplay",
  },
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
