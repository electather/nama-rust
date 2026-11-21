import { useUpdateBackupSettings } from "@/features/admin-backups/data/use-update-backup-settings";
import {
  backupSettingsErrors,
  typedBackupSettings,
} from "@/features/admin-backups/lib/backup-settings-form";
import type { BackupSettings } from "@/gen/nama/v1/server_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { FormField } from "@/shared/components/form-field/form-field";
import { onSubmitted } from "@/shared/lib/forms";

/** The form an Admin sets where Backups go and how many are kept with, showing the `saved` ones. */
export function BackupSettingsForm({ saved }: { saved: BackupSettings }) {
  const update = useUpdateBackupSettings();
  const errors = backupSettingsErrors(update.error);
  return (
    <form
      className="mt-10 flex max-w-md flex-col gap-5"
      onSubmit={onSubmitted((typed) => {
        update.mutate({ settings: typedBackupSettings(typed) });
      })}
    >
      <FormField
        label="Backup folder"
        name="backupFolder"
        autoComplete="off"
        spellCheck={false}
        placeholder="/mnt/backups/nama"
        required={false}
        defaultValue={saved.folder}
        hint="Where Backups are written, such as a share mounted from another machine, so a copy survives this one’s disk. Saving creates it if it’s missing. Leave it empty for the backups folder in the Server’s data folder."
        error={errors.backupFolder}
      />
      <FormField
        label="Backups kept"
        name="backupsKept"
        type="number"
        min={1}
        required={false}
        defaultValue={saved.keep}
        hint="After each Backup, the oldest the Server wrote beyond this many are deleted. Other files in the folder, such as a Backup you wrote yourself under another name, are never deleted. Leave it empty for 7."
        error={errors.backupsKept}
      />
      <FormEnd
        action={update.isPending ? "Saving Backup settings…" : "Save Backup settings"}
        variant="ghost"
        pending={update.isPending}
        done={update.isSuccess ? "Saved." : undefined}
        refusal={errors.form}
      />
    </form>
  );
}
