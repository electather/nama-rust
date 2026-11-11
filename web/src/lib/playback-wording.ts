import type { GenEnum } from "@bufbuild/protobuf/codegenv2";

import { HdrFormat, HdrFormatSchema } from "@/gen/nama/v1/browse_pb";
import {
  Container,
  ContainerSchema,
  PlaybackLimit,
  PlaybackLimitSchema,
  PlaybackMethod,
  PlaybackMethodSchema,
  type PlaybackReason,
  PlaybackReasonKind,
} from "@/gen/nama/v1/playback_pb";
import { HardwareAcceleration, HardwareAccelerationSchema } from "@/gen/nama/v1/server_pb";
import { channelsName, codecName } from "@/lib/browse";

// The one place playback is worded, so a translation library plugs in here.

/** Each Playback Method by name, and what to call a plan with none. */
export const methodNames = new Map<PlaybackMethod, string>([
  [PlaybackMethod.UNSPECIFIED, "Can’t play here"],
  [PlaybackMethod.DIRECT_PLAY, "Direct Play"],
  [PlaybackMethod.REMUX, "Remux"],
  [PlaybackMethod.TRANSCODE, "Transcode"],
]);

/** The Playback Limits below Original, largest first, by the names the player's menu shows. */
export const belowOriginal = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_1080P, name: "1080p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_720P, name: "720p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_480P, name: "480p" },
];

const hardwareNames = new Map<HardwareAcceleration, string>([
  [HardwareAcceleration.UNSPECIFIED, "Automatic"],
  [HardwareAcceleration.OFF, "Off"],
  [HardwareAcceleration.VAAPI, "VAAPI"],
  [HardwareAcceleration.QSV, "Intel QSV"],
  [HardwareAcceleration.NVENC, "NVIDIA NVENC"],
  [HardwareAcceleration.VIDEOTOOLBOX, "Apple VideoToolbox"],
]);

/** A hardware acceleration API's name in plain words, as the Server's own messages say it. */
export function hardwareName(api: HardwareAcceleration) {
  return hardwareNames.get(api) ?? `API ${api}`;
}

/** The texts of `reasons` that decided the plan, and of those that are only context. */
export function wordReasons(reasons: PlaybackReason[]) {
  const worded = (decided: boolean) =>
    reasons.filter((reason) => reason.decided === decided).map((reason) => reasonText(reason));
  return { decided: worded(true), context: worded(false) };
}

/**
 * A Playback Reason in this app's words, from its kind and params, or else the Server's `text`:
 * for a kind this app doesn't know, or a reason missing a param its sentence needs.
 */
function reasonText({ kind, params, text }: PlaybackReason) {
  const wording = wordings[kind];
  if (wording === undefined) return text;
  try {
    return wording(named(params));
  } catch (error) {
    if (error instanceof Unworded) return text;
    throw error;
  }
}

type Wording = (param: ReturnType<typeof named>) => string;

/** A sentence for each kind: one added to the schema without one fails the type check. */
const wordings: Record<PlaybackReasonKind, Wording | undefined> = {
  [PlaybackReasonKind.UNSPECIFIED]: undefined,
  [PlaybackReasonKind.TRACKS_UNKNOWN]: () =>
    "Its Tracks aren’t known, so it can only be Direct Played.",
  [PlaybackReasonKind.LENGTH_UNKNOWN]: () =>
    "Its length isn’t known, which a Remux or a Transcode needs, so it can only be Direct Played.",
  [PlaybackReasonKind.DEVICE_CANT_DECODE_CONTAINER]: (param) =>
    `This Device can’t open ${param.container()} files, so it can’t be Direct Played.`,
  [PlaybackReasonKind.DEVICE_CANT_DECODE_VIDEO]: (param) =>
    `This Device can’t decode ${param.codec()} video, so it can’t be Direct Played or Remuxed.`,
  [PlaybackReasonKind.DEVICE_CANT_DECODE_AUDIO]: (param) =>
    `This Device can’t decode ${param.codec()} audio, so it can’t be Direct Played.`,
  [PlaybackReasonKind.DEVICE_CANT_SHOW_HDR]: (param) =>
    `This Device can’t show ${param.hdr()}, so it can’t be Direct Played or Remuxed.`,
  [PlaybackReasonKind.DEVICE_CANT_PLAY_HLS]: () =>
    "This Device can’t play HLS, which a Remux and a Transcode need.",
  [PlaybackReasonKind.SERVER_LACKS_FFMPEG]: () =>
    "The Server lacks ffmpeg or ffprobe, so it can only Direct Play.",
  [PlaybackReasonKind.SERVER_CANT_ENCODE_VIDEO]: () =>
    "The Server’s ffmpeg lacks libx264, so it can’t Transcode.",
  [PlaybackReasonKind.DOLBY_VISION_NEEDS_BASE_LAYER]: () =>
    "The Server can’t Transcode Dolby Vision with no Base Layer, as in profile 5.",
  [PlaybackReasonKind.METHOD_FAILED_ON_DEVICE]: (param) =>
    `${param.method()} failed on this Device.`,
  [PlaybackReasonKind.SERVER_BUSY]: (param) =>
    `The Server is running as many Transcodes as it may at once (${param.count("maxTranscodes")}), so it can’t Transcode this until one ends.`,
  [PlaybackReasonKind.CHOSEN_AUDIO_NOT_FILES_OWN]: () =>
    "The audio Track chosen isn’t the one browsers play from the file, so it can’t be Direct Played.",
  [PlaybackReasonKind.PICTURE_SCALED_DOWN]: (param) =>
    `Its picture is larger than the ${param.limit()} Playback Limit, so a Transcode scales it down to ${param.count("widthPixels")}×${param.count("heightPixels")}.`,
  [PlaybackReasonKind.SUBTITLES_BURNED_IN]: () =>
    "This Device can’t draw the subtitles chosen, so a Transcode burns them into the picture.",
  [PlaybackReasonKind.DEVICE_PLAYS_FEWER_CHANNELS]: (param) =>
    `Its audio is converted to ${param.channels()}, the most this Device plays.`,
  [PlaybackReasonKind.DEVICE_CHANNELS_UNKNOWN]: (param) =>
    `Its audio is converted to ${param.channels()}, as this Device doesn’t say it plays more.`,
  [PlaybackReasonKind.CONVERSION_CHANNELS_CAPPED]: (param) =>
    `Its audio is converted to ${param.channels()}, the most the conversion makes.`,
  [PlaybackReasonKind.HDR_TONE_MAPPED]: (param) =>
    `Its ${param.hdr()} picture is converted to SDR, which this Device shows.`,
  [PlaybackReasonKind.HDR_NOT_TONE_MAPPED]: (param) =>
    `Nothing on the Server converts its ${param.hdr()} picture to SDR, so its colours look washed out.`,
  [PlaybackReasonKind.REMOTE_DEFAULT_LIMIT]: (param) =>
    `This Device is outside the local network, so the default Playback Limit for remote playback, ${param.limit()}, applies.`,
  [PlaybackReasonKind.PICTURE_SIZE_UNKNOWN]: (param) =>
    `Its picture size isn’t known, so it’s taken to fit the ${param.limit()} Playback Limit.`,
  [PlaybackReasonKind.CHOSEN_AUDIO_DROPPED]: () =>
    "The audio Track chosen can’t play here, so the file’s own plays instead.",
  [PlaybackReasonKind.PLAYBACK_LIMIT_DROPPED]: (param) =>
    `The ${param.limit()} Playback Limit can’t be kept to here, so it plays at Original.`,
  [PlaybackReasonKind.CHOSEN_SUBTITLES_DROPPED]: () =>
    "The subtitles chosen can’t be burned in here, so they’re left out.",
  [PlaybackReasonKind.TRANSCODES_ON_HARDWARE]: (param) =>
    `It Transcodes on the GPU with ${param.hardware()}.`,
  [PlaybackReasonKind.HARDWARE_ACCELERATION_OFF]: () =>
    "Hardware acceleration is off, so it Transcodes in software.",
  [PlaybackReasonKind.NO_HARDWARE_ACCELERATION_PASSED]: () =>
    "No hardware acceleration passed its test when the Server started, so it Transcodes in software.",
  [PlaybackReasonKind.HARDWARE_ACCELERATION_FAILED_TEST]: (param) =>
    `${param.hardware()} didn’t pass its test when the Server started, so it Transcodes in software.`,
  [PlaybackReasonKind.HARDWARE_FAILED_PARTWAY]: (param) =>
    `${param.hardware()} failed partway, so the rest Transcodes in software.`,
};

/** Thrown by a wording whose reason lacks a param it needs, or holds one this app can't name. */
class Unworded extends Error {}

function unworded(): never {
  throw new Unworded();
}

/** A reason's `params`, each named the way the rest of the app names it. */
function named(params: Record<string, string>) {
  const param = (key: string) => params[key] ?? unworded();
  return {
    codec: () => codecName(param("codec")),
    channels: () => channelsName(count(param("channels"))),
    container: () => containerName(param("container")),
    hdr: () => nameOf(HdrFormatSchema, hdrNames, param("hdrFormat")),
    limit: () => nameOf(PlaybackLimitSchema, limitNames, param("playbackLimit")),
    method: () => nameOf(PlaybackMethodSchema, methodNames, param("method")),
    hardware: () =>
      nameOf(HardwareAccelerationSchema, hardwareNames, param("hardwareAcceleration")),
    count: (key: string) => count(param(key)),
  };
}

function count(decimal: string) {
  const number = Number(decimal);
  return Number.isInteger(number) && number > 0 ? number : unworded();
}

/** What `names` calls the value of `schema` a param holds by its name there (`PLAYBACK_LIMIT_720P`). */
function nameOf<T extends number>(schema: GenEnum<T>, names: Map<T, string>, param: string) {
  for (const [value, name] of names) if (schema.value[value].name === param) return name;
  return unworded();
}

const containerNames = new Map<Container, string>([
  [Container.MP4, "MP4"],
  [Container.WEBM, "WebM"],
  [Container.MATROSKA, "Matroska"],
]);

/** A `Container` value's name, or else ffprobe's name for one the schema lacks, in capitals. */
function containerName(name: string) {
  if (!name.startsWith("CONTAINER_")) return name.toUpperCase();
  return nameOf(ContainerSchema, containerNames, name);
}

const hdrNames = new Map<HdrFormat, string>([
  [HdrFormat.HDR10, "HDR10"],
  [HdrFormat.HLG, "HLG"],
  [HdrFormat.DOLBY_VISION, "Dolby Vision"],
]);

const limitNames = new Map(belowOriginal.map(({ limit, name }) => [limit, name]));
