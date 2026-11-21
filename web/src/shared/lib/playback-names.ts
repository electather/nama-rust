import { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import { type FoundHardware, HardwareAcceleration } from "@/gen/nama/v1/server_pb";

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
function hardwareName(api: HardwareAcceleration) {
  return hardwareNames.get(api) ?? `API ${api}`;
}

/** What Transcodes may run on: an API, on a GPU if one is named. */
export type Hardware = Pick<FoundHardware, "api" | "gpu">;

/** An API on a GPU, such as "AMD Radeon RX 9070 XT · VAAPI", or the API alone without one. */
export function hardwareLabel({ api, gpu }: Hardware) {
  const label = gpu === undefined ? hardwareName(api) : `${gpu.name} · ${hardwareName(api)}`;
  // A GPU's name may start lowercase, to sit mid-sentence in the Server's reasons.
  return label.charAt(0).toUpperCase() + label.slice(1);
}
