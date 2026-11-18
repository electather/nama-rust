import {
  type FoundHardware,
  HardwareAcceleration,
  type PlaybackSettings,
} from "@/gen/nama/v1/server_pb";
import { hardwareName } from "@/shared/lib/playback-names";

/** What Transcodes may run on: an API, on a GPU if one is named. */
export type Hardware = Pick<FoundHardware, "api" | "gpu">;

/** An API on a GPU, such as "AMD Radeon RX 9070 XT · VAAPI", or the API alone without one. */
export function hardwareLabel({ api, gpu }: Hardware) {
  const label = gpu === undefined ? hardwareName(api) : `${gpu.name} · ${hardwareName(api)}`;
  // A GPU's name may start lowercase, to sit mid-sentence in the Server's reasons.
  return label.charAt(0).toUpperCase() + label.slice(1);
}

/**
 * Whether `choice` is what `saved` chose: its API on its GPU, or, saved with no GPU, on the first
 * GPU the API passed on, which saving then stores.
 */
export function isSaved(choice: Hardware, { hardwareAcceleration, gpu }: PlaybackSettings) {
  return choice.api === hardwareAcceleration && (gpu === undefined || choice.gpu?.id === gpu.id);
}

/** A saved choice that didn't pass its test this time, marked if its GPU is named. */
export function notFoundLabel(saved: Hardware) {
  const label = hardwareLabel(saved);
  return saved.gpu === undefined ? label : `${label} (not found)`;
}

/** The `<option>` value that stands for `hardware`: its API, and its GPU's id if it has one. */
export function hardwareValue({ api, gpu }: Hardware) {
  return gpu === undefined ? String(api) : `${api}@${gpu.id}`;
}

/** The hardware acceleration, and its GPU if any, that the option `value` stands for. */
export function typedHardware(value: string) {
  const [api, gpuId] = value.split("@");
  const hardwareAcceleration =
    Object.values(HardwareAcceleration).find((known) => String(known) === api) ??
    HardwareAcceleration.UNSPECIFIED;
  // The Server names the GPU, so only its id is sent.
  return { hardwareAcceleration, ...(gpuId !== undefined && { gpu: { id: gpuId } }) };
}
