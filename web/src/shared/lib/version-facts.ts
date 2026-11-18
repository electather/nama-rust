import { HdrFormat, type Track, type Version, type VideoTrack } from "@/gen/nama/v1/browse_pb";
import { metadataLine } from "@/shared/lib/title-facts";

/**
 * A Version's picture and sound in a few words, such as "4K · HDR · 5.1": its first video Track's
 * size and whether it's HDR, and its default audio Track's channels, or its first's. Empty until
 * the Server has probed it.
 */
export function versionLine({ tracks }: Version) {
  const [video] = tracks.flatMap(({ kind }) => (kind.case === "video" ? [kind.value] : []));
  const hdr = video && video.hdr !== HdrFormat.UNSPECIFIED ? "HDR" : undefined;
  return metadataLine(video && pictureOf(video), hdr, soundOf(tracks));
}

/** Picture sizes by name, largest first, with the width or height a picture reaches to be one. */
const pictureSizes = [
  // A film wider than 16:9 is shorter than its name, such as 3840×1600 for 4K, so width counts.
  { name: "4K", width: 3200, height: 2000 },
  { name: "1080p", width: 1800, height: 1000 },
  { name: "720p", width: 1200, height: 700 },
  { name: "SD", width: 1, height: 1 },
];

/** A video Track's picture size by name, such as "4K" or "1080p". */
export function pictureOf(video: VideoTrack) {
  return pictureSizes.find(({ width, height }) => video.width >= width || video.height >= height)
    ?.name;
}

const soundNames = new Map([
  [1, "Mono"],
  [2, "Stereo"],
  [6, "5.1"],
  [8, "7.1"],
]);

/**
 * The sound of `tracks`' default audio Track, or else its first, by its usual name, such as
 * "Stereo" or "5.1", or else how many channels it has.
 */
export function soundOf(tracks: Track[]) {
  const audio = tracks.flatMap(({ kind, default: isDefault }) =>
    kind.case === "audio" && kind.value.channels > 0
      ? [{ channels: kind.value.channels, isDefault }]
      : [],
  );
  const [first] = audio;
  const chosen = audio.find(({ isDefault }) => isDefault) ?? first;
  return chosen && channelsName(chosen.channels);
}

/** How many `channels` sound has, by its usual name, such as "Stereo" or "5.1". */
export function channelsName(channels: number) {
  return soundNames.get(channels) ?? `${channels} channels`;
}
