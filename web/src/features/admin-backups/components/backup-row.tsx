import { backupSize } from "@/features/admin-backups/lib/backup-facts";
import { At } from "@/features/admin-jobs/components/at";
import type { Backup } from "@/gen/nama/v1/server_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";

/** One Backup: its file name, when it was written and its size. */
export function BackupRow({ backup }: { backup: Backup }) {
  return (
    <AdminRow end={<span className="text-sm text-muted-foreground">{backupSize(backup)}</span>}>
      <span className="truncate font-medium">{backup.name}</span>
      <span className="text-sm text-muted-foreground">
        <At ms={backup.modifiedAtMs} />
      </span>
    </AdminRow>
  );
}
