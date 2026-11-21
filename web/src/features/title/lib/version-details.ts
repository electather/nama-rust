import { listed } from "@/features/title/lib/listed";
import { hdrBadge } from "@/features/title/lib/title-badges";
import type { AudioTrack, Track, Version } from "@/gen/nama/v1/browse_pb";
import { languageName } from "@/shared/lib/language-name";
import { sizeOf } from "@/shared/lib/title-facts";
import { channelsName, pictureOf } from "@/shared/lib/version-facts";

/** Codecs by the names people know them by; any other is shown in capitals. */
const codecNames = new Map([
  ["hevc", "HEVC"],
  ["h264", "H.264"],
  ["av1", "AV1"],
  ["vp9", "VP9"],
  ["mpeg2video", "MPEG-2"],
  ["mpeg4", "MPEG-4"],
]);

/** A codec by the name people know it by, such as "HEVC" for `hevc`. */
function codecName(codec: string) {
  return codec === "" ? undefined : (codecNames.get(codec) ?? codec.toUpperCase());
}

const tracksOf = (versions: Version[]) => versions.flatMap(({ tracks }) => tracks);

/** `versions`' picture sizes, HDR and video codecs, such as "4K, 1080p, Dolby Vision, HEVC". */
export function videoOf(versions: Version[]) {
  const videos = tracksOf(versions).flatMap(({ kind, codec }) =>
    kind.case === "video" ? [{ ...kind.value, codec }] : [],
  );
  const hdr = new Set(videos.map((video) => video.hdr));
  return listed([
    ...videos.map((video) => pictureOf(video)),
    hdrBadge(hdr),
    ...videos.map(({ codec }) => codecName(codec)),
  ]);
}

/**
 * `versions`' audio languages, each with its sound and whether it's Dolby Atmos or Audio
 * Description, such as "English (7.1, Dolby Atmos), French (Stereo, Audio Description)".
 */
export function audioOf(versions: Version[]) {
  return listed(tracksOf(versions).flatMap((track) => audioName(track)));
}

function audioName({ kind, language }: Track): string[] {
  const name = languageName(language);
  if (kind.case !== "audio" || name === undefined) {
    return [];
  }
  const marks = audioMarks(kind.value);
  return [marks === undefined ? name : `${name} (${marks})`];
}

/** An audio Track's sound and whether it's Dolby Atmos or Audio Description, such as "7.1, Dolby Atmos". */
function audioMarks({ channels, atmos, audioDescription }: AudioTrack) {
  return listed([
    channels > 0 ? channelsName(channels) : undefined,
    atmos ? "Dolby Atmos" : undefined,
    audioDescription ? "Audio Description" : undefined,
  ]);
}

/** `versions`' subtitle languages, marking SDH, such as "English (SDH), French". */
export function subtitlesOf(versions: Version[]) {
  return listed(tracksOf(versions).flatMap((track) => subtitleName(track)));
}

function subtitleName({ kind, language, sdh }: Track): string[] {
  const name = languageName(language);
  if (kind.case !== "subtitle" || name === undefined) {
    return [];
  }
  return [sdh ? `${name} (SDH)` : name];
}

/** How much room `versions` take on the Server together, such as "71.4 GB"; nothing if unknown. */
export function sizeOfAll(versions: Version[]) {
  let total = 0n;
  for (const { size } of versions) {
    total += size;
  }
  return total > 0n ? sizeOf(total) : undefined;
}
