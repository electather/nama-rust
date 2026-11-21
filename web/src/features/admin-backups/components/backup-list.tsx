import { BackupRow } from "@/features/admin-backups/components/backup-row";
import type { Backup } from "@/gen/nama/v1/server_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/** The Backups in the Backup folder, newest first, or a line saying there are none yet. */
export function BackupList({ backups }: { backups: Backup[] }) {
  if (backups.length === 0) {
    return <p className="mt-8 text-muted-foreground">No Backups yet.</p>;
  }
  return (
    <AdminList>
      {backups.map((backup) => (
        <BackupRow key={backup.name} backup={backup} />
      ))}
    </AdminList>
  );
}
