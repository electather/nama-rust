import type { GenEnum } from "@bufbuild/protobuf/codegenv2";

import { HdrFormat, HdrFormatSchema } from "@/gen/nama/v1/browse_pb";
import {
  Container,
  ContainerSchema,
  PlaybackLimitSchema,
  PlaybackMethod,
  PlaybackMethodSchema,
  type PlaybackReason,
  PlaybackReasonKind,
} from "@/gen/nama/v1/playback_pb";
import { HardwareAccelerationSchema } from "@/gen/nama/v1/server_pb";
import { codecName } from "@/shared/lib/codec-name";
import { belowOriginal, hardwareNames } from "@/shared/lib/playback-names";
import { channelsName } from "@/shared/lib/version-facts";

// Playback Reasons, and the names of methods, Playback Limits and hardware they use, are worded
// here and in `@/shared/lib/playback-names`, so a translation library plugs in there later (#690).

const methodNames = new Map<PlaybackMethod, string>([
  [PlaybackMethod.DIRECT_PLAY, "Direct Play"],
  [PlaybackMethod.REMUX, "Remux"],
  [PlaybackMethod.TRANSCODE, "Transcode"],
]);

/** How a plan plays by `method`, by name, or that it can't play here. */
export function methodName(method: PlaybackMethod) {
  return methodNames.get(method) ?? "Can’t play here";
}

/** Why a Version can’t play when the Server failed to work out its plan. */
export const serverFailedToPlan = "The Server couldn’t get it ready to play.";

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
export function reasonText({ kind, params, text }: PlaybackReason) {
  const wording = wordings[kind];
  if (wording === undefined) {
    return text;
  }
  try {
    return wording(named(params));
  } catch (error) {
    if (error instanceof UnwordedError) {
      return text;
    }
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
  [PlaybackReasonKind.DEVICE_CANT_DECODE_VIDEO]: (param) => {
    const depth = param.has("bitDepth") ? `${param.count("bitDepth")}-bit ` : "";
    return `This Device can’t decode ${depth}${param.codec()} video, so it can’t be Direct Played or Remuxed.`;
  },
  [PlaybackReasonKind.DEVICE_CANT_DECODE_AUDIO]: (param) =>
    `This Device can’t decode ${param.codec()} audio, so it can’t be Direct Played.`,
  [PlaybackReasonKind.DEVICE_CANT_SHOW_HDR]: (param) =>
    `This Device can’t show ${param.hdr()}, so it can’t be Direct Played or Remuxed.`,
  [PlaybackReasonKind.DEVICE_CANT_PLAY_HLS]: () =>
    "This Device can’t play the stream a Remux or a Transcode sends.",
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
  [PlaybackReasonKind.SUBTITLES_FILE_CARRIES_NO_FONTS]: () =>
    "The subtitles chosen need fonts their file doesn’t carry, so a Transcode burns them into the picture.",
  [PlaybackReasonKind.SUBTITLES_TRACK_CARRIES_NO_PALETTE]: () =>
    "The subtitles chosen carry no palette to draw them with, so a Transcode burns them into the picture.",
  [PlaybackReasonKind.KEYFRAMES_UNLISTED]: () =>
    "The Server couldn’t list where its keyframes fall, which a Remux needs, so it can’t Remux it.",
  [PlaybackReasonKind.METHOD_FAILED_ON_SERVER]: (param) =>
    `${param.method()} failed on the Server: its ffmpeg stopped before writing anything.`,
  [PlaybackReasonKind.DEVICE_PLAYS_FEWER_CHANNELS]: (param) =>
    `Its audio is converted to ${param.codec()} ${param.channels()}, as this Device doesn’t play 5.1.`,
  [PlaybackReasonKind.DEVICE_CHANNELS_UNKNOWN]: (param) =>
    `Its audio is converted to ${param.codec()} ${param.channels()}, as this Device doesn’t say it plays more.`,
  [PlaybackReasonKind.CONVERSION_CHANNELS_CAPPED]: (param) =>
    `Its audio is converted to ${param.codec()} ${param.channels()}, the most a Remux or a Transcode makes.`,
  [PlaybackReasonKind.HDR_TONE_MAPPED]: (param) =>
    `Its ${param.hdr()} picture is converted to SDR, which this Device shows.`,
  [PlaybackReasonKind.HDR_NOT_TONE_MAPPED]: (param) =>
    `Nothing on the Server converts its ${param.hdr()} picture to SDR, so its colours look washed out.`,
  [PlaybackReasonKind.REMOTE_DEFAULT_LIMIT]: (param) =>
    `This Device is outside the local network, so it asks for the default Playback Limit for remote playback, ${param.limit()}.`,
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
class UnwordedError extends Error {
  public override name = "UnwordedError";
}

function unworded(): never {
  throw new UnwordedError();
}

/** A reason's `params`, each named the way the rest of the app names it. */
function named(params: Record<string, string>) {
  const raw = (key: string) => params[key] ?? unworded();
  return {
    codec: () => codecName(raw("codec")),
    channels: () => channelsName(count(raw("channels"))),
    container: () => containerName(raw("container")),
    hdr: () => nameOf(HdrFormatSchema, hdrNames, raw("hdrFormat")),
    limit: () => nameOf(PlaybackLimitSchema, limitNames, raw("playbackLimit")),
    method: () => nameOf(PlaybackMethodSchema, methodNames, raw("method")),
    hardware: () => nameOf(HardwareAccelerationSchema, hardwareNames, raw("hardwareAcceleration")),
    count: (key: string) => count(raw(key)),
    has: (key: string) => key in params,
  };
}

/** A count a param holds as a decimal, which must be positive. */
function count(decimal: string) {
  const number = Number(decimal);
  return Number.isInteger(number) && number > 0 ? number : unworded();
}

/** What `names` calls the value of `schema` a param holds by its name there (`PLAYBACK_LIMIT_720P`). */
function nameOf<T extends number>(schema: GenEnum<T>, names: Map<T, string>, param: string) {
  for (const [value, name] of names) {
    if (schema.value[value].name === param) {
      return name;
    }
  }
  return unworded();
}

const containerNames = new Map<Container, string>([
  [Container.MP4, "MP4"],
  [Container.WEBM, "WebM"],
  [Container.MATROSKA, "Matroska"],
]);

/** A `Container` value's name, or else ffprobe's name for one the schema lacks, in capitals. */
function containerName(name: string) {
  if (!name.startsWith("CONTAINER_")) {
    return name.toUpperCase();
  }
  return nameOf(ContainerSchema, containerNames, name);
}

/** HDR formats by the names people know them by. */
export const hdrNames = new Map<HdrFormat, string>([
  [HdrFormat.HDR10, "HDR10"],
  [HdrFormat.HLG, "HLG"],
  [HdrFormat.DOLBY_VISION, "Dolby Vision"],
]);

const limitNames = new Map(belowOriginal.map(({ limit, name }) => [limit, name]));
