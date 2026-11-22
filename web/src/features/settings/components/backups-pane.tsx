import { BackupSettingRows } from "@/features/admin-backups/components/backup-setting-rows";
import { KeepSafe } from "@/features/admin-backups/components/keep-safe";
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
import { BackupStatus } from "@/features/settings/components/backup-status";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SaveToast } from "@/features/settings/components/save-toast";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { refusedWords } from "@/features/settings/lib/refused-words";

/**
 * The Server's Backups, for an Admin: how they stand, with Back up now, then the Backups written,
 * each with how to restore it, then where they go and how many are kept, saved as they're changed.
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
  const written = (
    <SettingGroup setting="restore" title="Backups">
      <BackupItems backups={backups} folder={folder} />
    </SettingGroup>
  );
  const storage = (
    <SettingGroup title="Storage">
      <BackupSettingRows inUse={folder} values={values} refused={words} onEdit={edit} />
      <KeepSafe />
    </SettingGroup>
  );
  return (
    <>
      <PaneFrame section="backups">
        <BackupStatus job={job} newest={backups[0]} />
        {written}
        {storage}
      </PaneFrame>
      <SaveToast says={words.form} onDismiss={dismiss} />
    </>
  );
}
