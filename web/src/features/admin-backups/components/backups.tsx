import { BackUpNow } from "@/features/admin-backups/components/back-up-now";
import { BackupList } from "@/features/admin-backups/components/backup-list";
import { BackupSettingsForm } from "@/features/admin-backups/components/backup-settings-form";
import { CodeText } from "@/features/admin-backups/components/code-text";
import { RestoreNote } from "@/features/admin-backups/components/restore-note";
import { UpkeepDisabled } from "@/features/admin-backups/components/upkeep-disabled";
import { useBackUpJob } from "@/features/admin-backups/data/use-back-up-job";
import { useBackups } from "@/features/admin-backups/data/use-backups";
import { Page } from "@/shared/components/page";

/**
 * The Backups page: the Backups in the Backup folder, Back up now, where they go and how many are
 * kept, and how to restore one.
 */
export function Backups() {
  const { settings, folder, backups } = useBackups();
  const job = useBackUpJob();
  return (
    <Page heading="Backups">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        A Backup holds a copy of the Server’s database, its config file and its Custom Images, all a
        restore needs to put it back. It also holds every Plugin’s settings, API keys included, so
        keep the Backup folder as safe as the Server’s data folder.
      </p>
      <p className="mt-4 max-w-prose text-pretty">
        Backups are written to <CodeText>{folder}</CodeText>
      </p>
      {job === undefined ? <UpkeepDisabled /> : <BackUpNow job={job} />}
      <BackupList backups={backups} />
      <BackupSettingsForm saved={settings} />
      <RestoreNote folder={folder} newest={backups[0]?.name} />
    </Page>
  );
}
