import type { Track } from "@/gen/nama/v1/browse_pb";
import { metadataLine } from "@/shared/lib/title-facts";
import { pictureOf, soundOf } from "@/shared/lib/version-facts";

/**
 * A Track in a few words, such as "AAC · 5.1 · English · Commentary": its codec, picture size or
 * channels, language and title.
 */
export function trackLine(track: Track) {
  const { codec, language, title, kind } = track;
  return metadataLine(
    codecName(codec),
    kind.case === "video" ? pictureOf(kind.value) : soundOf([track]),
    language === undefined ? undefined : languageName(language),
    title,
  );
}

/** A codec, as ffprobe names it, by the name people know it by, or else in capitals. */
export function codecName(codec: string) {
  return codecNames.get(codec) ?? codec.toUpperCase();
}

const codecNames = new Map([
  ["h264", "H.264"],
  ["ac3", "Dolby Digital"],
  ["eac3", "Dolby Digital Plus"],
  ["truehd", "Dolby TrueHD"],
  ["dts", "DTS"],
  ["opus", "Opus"],
  ["vorbis", "Vorbis"],
  ["subrip", "SRT"],
  ["mov_text", "Text"],
  ["webvtt", "WebVTT"],
  ["hdmv_pgs_subtitle", "PGS"],
  ["dvd_subtitle", "VobSub"],
]);

const languageNames = new Intl.DisplayNames("en", { type: "language" });

/** A language code as the file tags it, such as `eng`, by name, or as it is if it isn't one. */
function languageName(code: string) {
  try {
    return languageNames.of(code) ?? code;
  } catch {
    // A file's tag can be anything, and `of` refuses what isn't a language code.
    return code;
  }
}
