import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { HardwareAcceleration } from "@/gen/nama/v1/server_pb";

/** The Playback Limits below Original, largest first, by the names the player's menu shows. */
export const belowOriginal = [
  { limit: PlaybackLimit.PLAYBACK_LIMIT_1080P, name: "1080p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_720P, name: "720p" },
  { limit: PlaybackLimit.PLAYBACK_LIMIT_480P, name: "480p" },
];

/** Each hardware acceleration API's name in plain words, as the Server's own messages say it. */
export const hardwareNames = new Map<HardwareAcceleration, string>([
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
