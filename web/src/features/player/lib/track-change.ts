import { hdrNames } from "@/features/player/lib/playback-wording";
import { codecName } from "@/features/player/lib/track-line";
import type { Track } from "@/gen/nama/v1/browse_pb";
import { pictureOf, soundOf } from "@/shared/lib/version-facts";

/**
 * A Track the plan plays, such as "H.264 720p SDR", and the file's own `from` it, such as "HEVC 4K
 * HDR10", when converting changed it.
 */
export function trackChange(played: Track, source: Track | undefined) {
  const to = trackWords(played);
  const from = source === undefined ? to : trackWords(source);
  return from === to ? { from: undefined, to } : { from, to: withSdr(to, played, source) };
}

/** A video Track's picture, such as "4K HDR10", or a blank for an audio one. */
export function pictureWords({ kind }: Track) {
  return kind.case === "video" ? join([pictureOf(kind.value), hdrOf(kind)]) : "";
}

function trackWords(track: Track) {
  const facts = track.kind.case === "video" ? pictureWords(track) : soundOf([track]);
  return join([codecName(track.codec), facts]);
}

// SDR is said only of a picture converted from HDR.
function withSdr(to: string, played: Track, source: Track | undefined) {
  const fromHdr = source !== undefined && hdrOf(source.kind) !== undefined;
  return fromHdr && hdrOf(played.kind) === undefined ? `${to} SDR` : to;
}

function hdrOf(kind: Track["kind"]) {
  return kind.case === "video" ? hdrNames.get(kind.value.hdr) : undefined;
}

function join(words: (string | undefined)[]) {
  return words.filter(Boolean).join(" ");
}
