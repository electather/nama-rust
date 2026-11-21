import type { ConnectError } from "@connectrpc/connect";

import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import { formText, refusal, serverMessage } from "@/shared/lib/forms";

/** The Backup settings the form holds `typed`. */
export function typedBackupSettings(typed: FormData) {
  const keep = formText(typed, "backupsKept");
  return {
    folder: formText(typed, "backupFolder"),
    // Empty leaves it unset, for the Server's default.
    ...(keep !== "" && { keep: Number(keep) }),
  };
}

/** Why the Server refused the Backup settings, under their field, or `{}` while it hasn't. */
export function backupSettingsErrors(error: ConnectError | null) {
  if (error === null) {
    return {};
  }
  return refusal(
    error,
    "save the Backup settings",
    {},
    {
      [RefusalReason.FOLDER_NOT_ABSOLUTE]: { backupFolder: serverMessage(error) },
      [RefusalReason.FOLDER_NOT_WRITABLE]: { backupFolder: serverMessage(error) },
      // The only field the Server refuses as missing here, a count of 0.
      [RefusalReason.FIELD_REQUIRED]: { backupsKept: serverMessage(error) },
    },
  );
}
