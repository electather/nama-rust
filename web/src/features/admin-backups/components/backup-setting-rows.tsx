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
        {...rowText(backupLabels, "folder")}
        saved={values.folder}
        error={refused.backupFolder}
        input={{ autoComplete: "off", spellCheck: false, placeholder: "/mnt/backups/nama" }}
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
