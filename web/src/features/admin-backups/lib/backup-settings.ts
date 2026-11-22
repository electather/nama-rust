import { RefusalReason } from "@/gen/nama/v1/refusal_pb";
import type { BackupSettings } from "@/gen/nama/v1/server_pb";
import { serverMessage } from "@/shared/lib/forms";
import type { SettingText } from "@/shared/lib/setting-text";

/** Where Backups go and how many are kept, as their fields hold them. */
export interface BackupValues {
  folder: string;
  /** Empty for the Server's default. */
  keep: string;
}

/**
 * What each of the Backups section's rows and groups says, in its order. The toast names a refused
 * change by its label.
 */
export const backupLabels: Record<keyof BackupValues | "schedule" | "restore", SettingText> = {
  schedule: { label: "Schedule", words: "daily automatic upkeep job" },
  folder: {
    label: "Backup folder",
    hint: "Where Backups are written, such as a share mounted from another machine, so a copy survives this one’s disk. Saving creates it if it’s missing. Leave it empty for the backups folder in the Server’s data folder.",
    words: "path",
  },
  keep: {
    label: "Backups kept",
    hint: "After each Backup, the oldest the Server wrote beyond this many are deleted. Other files in the folder, such as a Backup you wrote yourself under another name, are never deleted. Leave it empty for 7.",
    words: "retention",
  },
  restore: { label: "Restoring a Backup", words: "database" },
};

/** The settings as the Server has them `saved`. */
export function savedBackupValues(saved: BackupSettings): BackupValues {
  return { folder: saved.folder, keep: saved.keep?.toString() ?? "" };
}

/** What's wrong with how many Backups to keep, as `typed`, before the Server is asked. Empty is fine. */
export function keepProblem(typed: string) {
  // A guard behind the field, which keeps to whole numbers of 1 or more: a uint32 can't hold a
  // negative or a fraction.
  const keep = typed.trim();
  return keep === "" || /^[1-9]\d*$/u.test(keep) ? undefined : "Enter a whole number of 1 or more.";
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
