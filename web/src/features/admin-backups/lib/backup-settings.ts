import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { BackupSettings } from "@/gen/nama/v1/server_pb";
import { serverMessage } from "@/shared/lib/forms";

/** Where Backups go and how many are kept, as their fields hold them. */
export interface BackupValues {
  folder: string;
  /** Empty for the Server's default. */
  keep: string;
}

/** Each setting's name, as its row says it, and its pane's toast when a change to it is refused. */
export const backupLabels: Record<keyof BackupValues, string> = {
  folder: "Backup folder",
  keep: "Backups kept",
};

/** The settings as the Server has them `saved`. */
export function savedBackupValues(saved: BackupSettings): BackupValues {
  return { folder: saved.folder, keep: saved.keep?.toString() ?? "" };
}

/** The Backup settings to save for `values`. */
export function backupSettings({ folder, keep }: BackupValues) {
  // Empty leaves it unset, for the Server's default.
  return { folder, ...(keep !== "" && { keep: Number(keep) }) };
}

/** Which field the Server's `failure` to save the Backup settings is said under, by its reason. */
export function backupRefusalFields(failure: unknown) {
  const message = serverMessage(failure);
  return {
    [RefusalReason.FOLDER_NOT_ABSOLUTE]: { backupFolder: message },
    [RefusalReason.FOLDER_NOT_WRITABLE]: { backupFolder: message },
    // The only field the Server refuses as missing here, a count of 0.
    [RefusalReason.FIELD_REQUIRED]: { backupsKept: message },
  };
}
