import { BackupItem } from "@/features/settings/components/backup-item";
import type { Backup } from "@/gen/nama/v1/server_pb";

/** The Backups in the Backup folder, newest first, or a line saying there are none yet. */
export function BackupItems({ backups }: { backups: Backup[] }) {
  if (backups.length === 0) {
    return <p className="py-3.5 text-sm text-muted-foreground">No Backups yet.</p>;
  }
  return (
    <ul aria-label="Backups" className="flex flex-col">
      {backups.map((backup) => (
        <BackupItem key={backup.name} backup={backup} />
      ))}
    </ul>
  );
}
