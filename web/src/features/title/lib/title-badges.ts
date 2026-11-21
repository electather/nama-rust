import { HdrFormat, type Version } from "@/gen/nama/v1/browse_pb";

/**
 * What any of a title's `versions`, a Movie's or its Episodes', offers, by the names on its page:
 * "4K" for video 3840 wide or 2160 tall, "Dolby Vision" or else "HDR" from the video's `hdr`, "Atmos"
 * for Dolby Atmos audio, and "SDH" for SDH subtitles.
 */
export function titleBadges(versions: Version[]) {
  const tracks = versions.flatMap((version) => version.tracks);
  const videos = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  return [
    videos.some(({ width, height }) => width >= 3840 || height >= 2160) ? "4K" : undefined,
    hdrBadge(new Set(videos.map(({ hdr }) => hdr))),
    tracks.some(({ kind }) => kind.case === "audio" && kind.value.atmos) ? "Atmos" : undefined,
    tracks.some(({ kind, sdh }) => kind.case === "subtitle" && sdh) ? "SDH" : undefined,
  ];
}

/** "Dolby Vision", or else "HDR", if any of `formats` is one. */
export function hdrBadge(formats: Set<HdrFormat>) {
  if (formats.has(HdrFormat.DOLBY_VISION)) {
    return "Dolby Vision";
  }
  return formats.has(HdrFormat.HDR10) || formats.has(HdrFormat.HLG) ? "HDR" : undefined;
}
