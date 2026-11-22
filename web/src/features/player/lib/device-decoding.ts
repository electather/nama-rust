import {
  AudioCodec,
  Container,
  DecodingEvidence,
  SubtitleFormat,
  VideoCodec,
} from "@/gen/nama/v1/playback_pb";

/** The values of the generated enum `E` but its unspecified one, so a table keyed by them needs
 * an entry for each value the schema adds. */
type Named<E> = Exclude<E[keyof E], 0>;

/** The media type to ask the browser about for each container. */
const CONTAINERS: Record<Named<typeof Container>, string> = {
  [Container.MP4]: "video/mp4",
  [Container.WEBM]: "video/webm",
  [Container.MATROSKA]: "video/x-matroska",
};
// ponytail: one common profile per codec, so a 10-bit HEVC, VP9 or AV1 Track, or a 4K one, may
// pass on a Device that decodes only the 8-bit HD one; ask about that profile too, as about
// `H264_10BIT`, once a Device is seen failing one.
const VIDEO_CODECS: Record<Named<typeof VideoCodec>, string> = {
  [VideoCodec.H264]: 'video/mp4; codecs="avc1.640028"',
  [VideoCodec.HEVC]: 'video/mp4; codecs="hvc1.1.6.L120.90"',
  [VideoCodec.VP8]: 'video/webm; codecs="vp8"',
  [VideoCodec.VP9]: 'video/webm; codecs="vp09.00.40.08"',
  [VideoCodec.AV1]: 'video/mp4; codecs="av01.0.08M.08"',
};
/** H.264 High 10, asked about apart from High since most browsers decode only High. */
const H264_10BIT = 'video/mp4; codecs="avc1.6E0028"';
/** HEVC Main 10, which a Transcode of a 4K or HDR Version makes for a Device that decodes it. */
const HEVC_10BIT = 'video/mp4; codecs="hvc1.2.4.L153.B0"';
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
const FILM_4K = { ...FILM, width: 3840, height: 2160 };

/** The subtitle formats whose renderer failed on this page, which it no longer draws until a reload. */
const failedRenderers = new Set<SubtitleFormat>();

/**
 * Stops saying this Device draws subtitles in `format` until the page is reloaded, as when their
 * renderer failed, so the next plan burns them in or drops them (ADR 0033).
 */
export function stopDrawing(format: SubtitleFormat) {
  failedRenderers.add(format);
}

/** What this Device decodes, found out afresh: the Server never stores it. */
export async function findDeviceDecoding() {
  const video = document.createElement("video");
  const nativeHls = playsHlsNatively();
  const mse = "MediaSource" in globalThis || "ManagedMediaSource" in globalThis;
  // hls.js plays a Remux or Transcode through MSE, which may refuse a codec the browser decodes
  // from a file, as E-AC-3 and AC-3 can be. Native HLS doesn't use MSE.
  const types: MediaDecodingType[] = !nativeHls && mse ? ["file", "media-source"] : ["file"];
  const [videoCodecs, audioCodecs, h26410bit, hevc10bit] = await Promise.all([
    decodable(VIDEO_CODECS, types, (contentType) => ({ video: { contentType, ...FILM } })),
    decodable(AUDIO_CODECS, types, (contentType) => ({ audio: { contentType } })),
    decodes(types, { video: { contentType: H264_10BIT, ...FILM } }),
    decodes(types, { video: { contentType: HEVC_10BIT, ...FILM_4K } }),
  ]);
  return {
    containers: keysOf(CONTAINERS).filter(
      (container) => video.canPlayType(CONTAINERS[container]) !== "",
    ),
    videoCodecs,
    audioCodecs,
    hdr: matchMedia("(dynamic-range: high)").matches,
    h26410bit,
    hevc10bit,
    nativeHls,
    mse,
    audioChannels: audioChannels(),
    subtitleFormats: drawnFormats().filter((format) => !failedRenderers.has(format)),
    // A browser only says what it may play, and overstates (ADR 0027).
    evidence: DecodingEvidence.DECLARED,
  };
}

/** The subtitle formats this Device has what their renderers need to draw. */
function drawnFormats() {
  if (!("WebAssembly" in globalThis)) {
    return [];
  }
  const bitmaps = [SubtitleFormat.PGS, SubtitleFormat.VOBSUB];
  return startsJassub() ? [...bitmaps, SubtitleFormat.ASS] : bitmaps;
}

/**
 * Whether JASSUB, the ASS renderer, can start here. Its constructor throws without OffscreenCanvas,
 * and its worker, an ES module, fails to load where workers can't be modules. Either costs a second
 * plan to burn the Track in (ADR 0033).
 */
function startsJassub() {
  return (
    "Worker" in globalThis &&
    "transferControlToOffscreen" in HTMLCanvasElement.prototype &&
    knowsModuleWorkers()
  );
}

function knowsModuleWorkers() {
  let readsType = false;
  // Only a browser that knows module workers reads `type`. The URL loads nothing: Chrome throws for
  // it, after reading `type`, and any worker another browser starts is ended at once.
  try {
    new Worker("blob://", {
      get type() {
        readsType = true;
        return "module" as const;
      },
    }).terminate();
  } catch {
    // What a browser throws for the URL says nothing more than whether it read `type`.
  }
  return readsType;
}

/** How many audio channels this Device plays out at once, such as 6 for 5.1, or 0 if it can't say. */
function audioChannels() {
  if (!("AudioContext" in globalThis)) {
    return 0;
  }
  const context = new AudioContext();
  const channels = context.destination.maxChannelCount;
  void context.close();
  return channels;
}

/** Whether this browser's `<video>` plays an HLS playlist itself, without hls.js. */
export function playsHlsNatively() {
  return document.createElement("video").canPlayType("application/vnd.apple.mpegurl") !== "";
}

/** Those of `codecs` this Device decodes from each of `types`, as `media` describes each one's type. */
async function decodable<C extends number>(
  codecs: Record<C, string>,
  types: MediaDecodingType[],
  media: (contentType: string) => ConfigurationWithoutType,
) {
  const decoded = await Promise.all(
    keysOf(codecs).map(async (codec) =>
      (await decodes(types, media(codecs[codec]))) ? [codec] : [],
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

type ConfigurationWithoutType = Omit<MediaDecodingConfiguration, "type">;

/** Whether this Device decodes `media` from every one of `types`. */
async function decodes(types: MediaDecodingType[], media: ConfigurationWithoutType) {
  // A browser without MediaCapabilities, or that rejects the type, can't be counted on to decode it.
  try {
    const supported = await Promise.all(
      types.map(async (type) => {
        const info = await navigator.mediaCapabilities.decodingInfo({ ...media, type });
        return info.supported;
      }),
    );
    return supported.every(Boolean);
  } catch {
    return false;
  }
}
