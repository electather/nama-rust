import {
  hardwareChoices,
  hardwareValue,
  savedHardware,
  typedHardware,
} from "@/features/admin-playback/lib/hardware";
import { limitPicked } from "@/features/admin-playback/lib/remote-limits";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { PlaybackSettings } from "@/gen/nama/v1/server_pb";
import { type FormErrors, refusalOf, serverMessage } from "@/shared/lib/forms";
import type { Hardware } from "@/shared/lib/playback-names";

/** The Playback settings, as their fields hold them. */
export interface PlaybackValues {
  segmentFolder: string;
  keepBehindMinutes: string;
  /** The hardware option's value, as `hardwareValue` makes it. */
  hardwareAcceleration: string;
  maxTranscodes: string;
  remotePlaybackLimit: string;
}

/** Each setting's name, as its field says it and the toast names it when a change to it is refused. */
export const playbackLabels: Record<keyof PlaybackValues, string> = {
  segmentFolder: "Segment folder",
  keepBehindMinutes: "Keep for skipping back",
  hardwareAcceleration: "Hardware acceleration",
  maxTranscodes: "Transcodes at once",
  remotePlaybackLimit: "Default Playback Limit for remote playback",
};

/** The `saved` Playback settings as their fields hold them, given the hardware that `passed` its test. */
export function savedPlayback(saved: PlaybackSettings, passed: Hardware[]): PlaybackValues {
  return {
    segmentFolder: saved.segmentFolder,
    keepBehindMinutes: String(saved.keepBehindMinutes ?? ""),
    hardwareAcceleration: hardwareValue(savedHardware(saved, hardwareChoices(passed))),
    maxTranscodes: String(saved.maxTranscodes ?? ""),
    remotePlaybackLimit: String(saved.remotePlaybackLimit),
  };
}

/** The Playback settings to save for `values`, every one of them, as saving replaces them all. */
export function playbackSettings(values: PlaybackValues) {
  const { maxTranscodes, keepBehindMinutes } = values;
  return {
    segmentFolder: values.segmentFolder,
    // Empty leaves each unset, for the Server's default.
    ...(maxTranscodes !== "" && { maxTranscodes: Number(maxTranscodes) }),
    ...(keepBehindMinutes !== "" && { keepBehindMinutes: Number(keepBehindMinutes) }),
    ...typedHardware(values.hardwareAcceleration),
    remotePlaybackLimit: limitPicked(values.remotePlaybackLimit),
  };
}

/** Why the Server refused a Playback setting `error` is about, said under its field. */
export function playbackRefusals(error: unknown): Partial<Record<RefusalReason, FormErrors>> {
  return {
    [RefusalReason.FOLDER_NOT_ABSOLUTE]: { segmentFolder: serverMessage(error) },
    [RefusalReason.FOLDER_NOT_WRITABLE]: { segmentFolder: serverMessage(error) },
    // The only fields the Server refuses as missing here, a count of 0.
    [RefusalReason.FIELD_REQUIRED]:
      refusalOf(error).field === "settings.keep_behind_minutes"
        ? { keepBehindMinutes: serverMessage(error) }
        : { maxTranscodes: serverMessage(error) },
    [RefusalReason.GPU_NOT_OFFERED]: { hardwareAcceleration: serverMessage(error) },
  };
}
