/** The containers and codecs to ask the browser about, as `crates/nama-server/src/playback.rs`
 * names them. */
const CONTAINERS = ["video/mp4", "video/webm", "video/x-matroska"];
const VIDEO_CODECS = [
  'video/mp4; codecs="avc1.640028"',
  'video/mp4; codecs="hvc1.1.6.L120.90"',
  'video/webm; codecs="vp8"',
  'video/webm; codecs="vp09.00.40.08"',
  'video/mp4; codecs="av01.0.08M.08"',
];
const AUDIO_CODECS = [
  'audio/mp4; codecs="mp4a.40.2"',
  "audio/mpeg",
  'audio/mp4; codecs="ac-3"',
  'audio/mp4; codecs="ec-3"',
  'audio/webm; codecs="opus"',
  'audio/webm; codecs="vorbis"',
  'audio/mp4; codecs="flac"',
  'audio/mp4; codecs="dtsc"',
  'audio/mp4; codecs="mlpa"',
];
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
    contentTypes: [
      ...CONTAINERS.filter((type) => video.canPlayType(type) !== ""),
      ...videoCodecs,
      ...audioCodecs,
    ],
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

/** Those of `contentTypes` this Device decodes, as `configuration` asks about each. */
async function decodable(
  contentTypes: string[],
  configuration: (contentType: string) => MediaDecodingConfiguration,
) {
  const decoded = await Promise.all(
    contentTypes.map(async (contentType) =>
      (await decodes(configuration(contentType))) ? [contentType] : [],
    ),
  );
  return decoded.flat();
}

async function decodes(configuration: MediaDecodingConfiguration) {
  // A browser without MediaCapabilities, or that rejects the type, can't be counted on to decode it.
  try {
    return (await navigator.mediaCapabilities.decodingInfo(configuration)).supported;
  } catch {
    return false;
  }
}
