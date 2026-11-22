import { backupLabels, type BackupValues } from "@/features/admin-backups/lib/backup-settings";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";
import type { FormErrors } from "@/shared/lib/forms";

/** Where Backups go and how many are kept, each saved once its field is left. */
export function BackupSettingRows({
  values,
  refused,
  onEdit,
}: {
  values: BackupValues;
  /** Why the Server refused a change, under the field it's about. */
  refused: FormErrors;
  onEdit: (edits: Partial<BackupValues>) => void;
}) {
  return (
    <>
      <TypedSettingRow
        label={backupLabels.folder}
        hint="Where Backups are written, such as a share mounted from another machine, so a copy survives this one’s disk. Saving creates it if it’s missing. Leave it empty for the backups folder in the Server’s data folder."
        saved={values.folder}
        error={refused.backupFolder}
        input={{ autoComplete: "off", spellCheck: false, placeholder: "/mnt/backups/nama" }}
        onSave={(folder) => {
          onEdit({ folder });
        }}
      />
      <TypedSettingRow
        label={backupLabels.keep}
        hint="After each Backup, the oldest the Server wrote beyond this many are deleted. Other files in the folder, such as a Backup you wrote yourself under another name, are never deleted. Leave it empty for 7."
        saved={values.keep}
        error={refused.backupsKept}
        input={{ type: "number", inputMode: "numeric", min: 1, placeholder: "7" }}
        onSave={(keep) => {
          onEdit({ keep });
        }}
      />
    </>
  );
}
