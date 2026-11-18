import type { ConnectError } from "@connectrpc/connect";

import { typedHardware } from "@/features/admin-playback/lib/hardware";
import { limitPicked } from "@/features/admin-playback/lib/remote-limits";
import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal, refusalOf, serverMessage } from "@/shared/lib/forms";

/** The Playback settings the form holds `typed`. */
export function typedSettings(typed: FormData) {
  const maxTranscodes = formText(typed, "maxTranscodes");
  const keepBehindMinutes = formText(typed, "keepBehindMinutes");
  return {
    segmentFolder: formText(typed, "segmentFolder"),
    // Empty leaves each unset, for the Server's default.
    ...(maxTranscodes !== "" && { maxTranscodes: Number(maxTranscodes) }),
    ...(keepBehindMinutes !== "" && { keepBehindMinutes: Number(keepBehindMinutes) }),
    ...typedHardware(formText(typed, "hardwareAcceleration")),
    remotePlaybackLimit: limitPicked(formText(typed, "remotePlaybackLimit")),
  };
}

/** Why the Server refused the Playback settings, under their field, or `{}` while it hasn't. */
export function playbackSettingsErrors(error: ConnectError | null) {
  if (error === null) {
    return {};
  }
  return refusal(
    error,
    "save the playback settings",
    {},
    {
      [RefusalReason.FOLDER_NOT_ABSOLUTE]: { segmentFolder: serverMessage(error) },
      [RefusalReason.FOLDER_NOT_WRITABLE]: { segmentFolder: serverMessage(error) },
      // The only fields the Server refuses as missing here, a count of 0.
      [RefusalReason.FIELD_REQUIRED]:
        refusalOf(error).field === "settings.keep_behind_minutes"
          ? { keepBehindMinutes: serverMessage(error) }
          : { maxTranscodes: serverMessage(error) },
      [RefusalReason.GPU_NOT_OFFERED]: { hardwareAcceleration: serverMessage(error) },
    },
  );
}
