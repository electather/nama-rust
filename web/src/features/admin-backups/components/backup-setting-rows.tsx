import {
  backupLabels,
  type BackupValues,
  keepProblem,
} from "@/features/admin-backups/lib/backup-settings";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";
import type { FormErrors } from "@/shared/lib/forms";
import { rowText } from "@/shared/lib/setting-text";

/** Where Backups go and how many are kept, each saved once its field is left. */
export function BackupSettingRows({
  inUse,
  values,
  refused,
  onEdit,
}: {
  /** The folder Backups are written to now, shown while the field is empty. */
  inUse: string;
  values: BackupValues;
  /** Why the Server refused a change, under the field it's about. */
  refused: FormErrors;
  onEdit: (edits: Partial<BackupValues>) => void;
}) {
  return (
    <>
      <TypedSettingRow
        {...rowText(backupLabels, "folder")}
        saved={values.folder}
        error={refused.backupFolder}
        input={{ autoComplete: "off", spellCheck: false, placeholder: inUse }}
        onSave={(folder) => {
          onEdit({ folder });
        }}
      />
      <TypedSettingRow
        {...rowText(backupLabels, "keep")}
        saved={values.keep}
        error={refused.backupsKept}
        check={keepProblem}
        input={{ type: "number", inputMode: "numeric", min: 1, placeholder: "7" }}
        onSave={(keep) => {
          onEdit({ keep });
        }}
      />
    </>
  );
}
