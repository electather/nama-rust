import { AudioCodec, Container, VideoCodec } from "@/gen/nama/v1/playback_pb";

/** The values of the generated enum `E` but its unspecified one, so a table keyed by them needs
 * an entry for each value the schema adds. */
type Named<E> = Exclude<E[keyof E], 0>;

/** The media type to ask the browser about for each container. */
const CONTAINERS: Record<Named<typeof Container>, string> = {
  [Container.MP4]: "video/mp4",
  [Container.WEBM]: "video/webm",
  [Container.MATROSKA]: "video/x-matroska",
};
// ponytail: one common profile per codec, so a 10-bit or 4K Track may pass on a Device that
// decodes only the 8-bit HD one; ask about the Track's own profile once probing reads it.
const VIDEO_CODECS: Record<Named<typeof VideoCodec>, string> = {
  [VideoCodec.H264]: 'video/mp4; codecs="avc1.640028"',
  [VideoCodec.HEVC]: 'video/mp4; codecs="hvc1.1.6.L120.90"',
  [VideoCodec.VP8]: 'video/webm; codecs="vp8"',
  [VideoCodec.VP9]: 'video/webm; codecs="vp09.00.40.08"',
  [VideoCodec.AV1]: 'video/mp4; codecs="av01.0.08M.08"',
};
const AUDIO_CODECS: Record<Named<typeof AudioCodec>, string> = {
  [AudioCodec.AAC]: 'audio/mp4; codecs="mp4a.40.2"',
  [AudioCodec.MP3]: "audio/mpeg",
  [AudioCodec.AC3]: 'audio/mp4; codecs="ac-3"',
  [AudioCodec.EAC3]: 'audio/mp4; codecs="ec-3"',
  [AudioCodec.OPUS]: 'audio/webm; codecs="opus"',
  [AudioCodec.VORBIS]: 'audio/webm; codecs="vorbis"',
  [AudioCodec.FLAC]: 'audio/mp4; codecs="flac"',
  [AudioCodec.DTS]: 'audio/mp4; codecs="dtsc"',
  [AudioCodec.TRUEHD]: 'audio/mp4; codecs="mlpa"',
};
/** A 1080p film's video, which `decodingInfo` needs to judge a video codec. */
const FILM = { width: 1920, height: 1080, bitrate: 10_000_000, framerate: 24 };

/** What this Device decodes, found out afresh: the Server never stores it. */
export async function findDeviceDecoding() {
  const video = document.createElement("video");
  const [videoCodecs, audioCodecs] = await Promise.all([
    decodable(VIDEO_CODECS, (contentType) => ({ type: "file", video: { contentType, ...FILM } })),
    decodable(AUDIO_CODECS, (contentType) => ({ type: "file", audio: { contentType } })),
  ]);
  return {
    containers: keysOf(CONTAINERS).filter(
      (container) => video.canPlayType(CONTAINERS[container]) !== "",
    ),
    videoCodecs,
    audioCodecs,
    hdr: matchMedia("(dynamic-range: high)").matches,
    nativeHls: playsHlsNatively(),
    mse: "MediaSource" in window || "ManagedMediaSource" in window,
    audioChannels: audioChannels(),
  };
}

/** How many audio channels this Device plays out at once, such as 6 for 5.1, or 0 if it can't say. */
function audioChannels() {
  if (!("AudioContext" in window)) return 0;
  const context = new AudioContext();
  const channels = context.destination.maxChannelCount;
  void context.close();
  return channels;
}

/** Whether this browser's `<video>` plays an HLS playlist itself, without hls.js. */
export function playsHlsNatively() {
  return document.createElement("video").canPlayType("application/vnd.apple.mpegurl") !== "";
}

/** Those of `codecs` this Device decodes, as `configuration` asks about each one's media type. */
async function decodable<C extends number>(
  codecs: Record<C, string>,
  configuration: (contentType: string) => MediaDecodingConfiguration,
) {
  const decoded = await Promise.all(
    keysOf(codecs).map(async (codec) =>
      (await decodes(configuration(codecs[codec]))) ? [codec] : [],
    ),
  );
  return decoded.flat();
}

/** The enum values `table` is keyed by. */
function keysOf<C extends number>(table: Record<C, string>) {
  // `Object.keys` gives every key as a string, even a number; the filter, true of each, only
  // tells TypeScript so without a cast.
  return Object.keys(table)
    .map(Number)
    .filter((key): key is C => key in table);
}

async function decodes(configuration: MediaDecodingConfiguration) {
  // A browser without MediaCapabilities, or that rejects the type, can't be counted on to decode it.
  try {
    return (await navigator.mediaCapabilities.decodingInfo(configuration)).supported;
  } catch {
    return false;
  }
}
