import { BackUpNow } from "@/features/admin-backups/components/back-up-now";
import { BackUpWhen } from "@/features/admin-backups/components/back-up-when";
import { BackupSettingRows } from "@/features/admin-backups/components/backup-setting-rows";
import { BackupsAbout } from "@/features/admin-backups/components/backups-about";
import { RestoreNote } from "@/features/admin-backups/components/restore-note";
import { useBackUpJob } from "@/features/admin-backups/data/use-back-up-job";
import { useBackups } from "@/features/admin-backups/data/use-backups";
import { useUpdateBackupSettings } from "@/features/admin-backups/data/use-update-backup-settings";
import {
  backupLabels,
  backupRefusalFields,
  backupSettings,
  savedBackupValues,
} from "@/features/admin-backups/lib/backup-settings";
import { BackupItems } from "@/features/settings/components/backup-items";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SaveToast } from "@/features/settings/components/save-toast";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * The Server's Backups, for an Admin: Back up now, when Upkeep backs up by itself, where Backups go
 * and how many are kept, saved as they're changed, the Backups written and how to restore one.
 */
export function BackupsPane() {
  const { settings, folder, backups } = useBackups();
  const job = useBackUpJob();
  const { mutateAsync } = useUpdateBackupSettings();
  const { values, refused, edit, dismiss } = useAutoSave(
    savedBackupValues(settings),
    async (wanted) => {
      await mutateAsync({ settings: backupSettings(wanted) });
    },
  );
  const words = refusedWords(
    refused,
    backupLabels,
    refused && backupRefusalFields(refused.failure),
  );
  const schedule = (
    <SettingGroup setting="schedule" title={backupLabels.schedule.label}>
      <BackUpWhen job={job} />
    </SettingGroup>
  );
  const storage = (
    <SettingGroup title="Storage">
      <BackupSettingRows values={values} refused={words} onEdit={edit} />
    </SettingGroup>
  );
  const written = (
    <SettingGroup title="In the folder">
      <BackupItems backups={backups} />
    </SettingGroup>
  );
  const restoring = (
    <SettingGroup setting="restore" title={backupLabels.restore.label}>
      <RestoreNote folder={folder} newest={backups[0]?.name} />
    </SettingGroup>
  );
  return (
    <>
      <PaneFrame section="backups" action={job && <BackUpNow job={job} />}>
        <BackupsAbout folder={folder} />
        {schedule}
        {storage}
        {written}
        {restoring}
      </PaneFrame>
      <SaveToast says={words.form} onDismiss={dismiss} />
    </>
  );
}
