import { HardwareAcceleration, type PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { type Hardware, hardwareLabel } from "@/shared/lib/playback-names";

/** What Transcodes may run on: Automatic, Off, then each API on each GPU that `passed` its test. */
export function hardwareChoices(passed: Hardware[]): Hardware[] {
  return [{ api: HardwareAcceleration.UNSPECIFIED }, { api: HardwareAcceleration.OFF }, ...passed];
}

/**
 * Whether `choice` is what `saved` chose: its API on its GPU, or, saved with no GPU, on the first
 * GPU the API passed on, which saving then stores.
 */
export function isSaved(choice: Hardware, { hardwareAcceleration, gpu }: PlaybackSettings) {
  return choice.api === hardwareAcceleration && (gpu === undefined || choice.gpu?.id === gpu.id);
}

/** What `saved` chose: one of `choices`, or else as it was saved, when it didn't pass its test. */
export function savedHardware(saved: PlaybackSettings, choices: Hardware[]): Hardware {
  return (
    choices.find((choice) => isSaved(choice, saved)) ?? {
      api: saved.hardwareAcceleration,
      gpu: saved.gpu,
    }
  );
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
