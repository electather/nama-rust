import { Archive } from "lucide-react";

import { backupSize } from "@/features/admin-backups/lib/backup-facts";
import { At } from "@/features/admin-jobs/components/at";
import { IconTile } from "@/features/settings/components/icon-tile";
import { ItemRow } from "@/features/settings/components/item-row";
import type { Backup } from "@/gen/nama/v1/server_pb";

/** One Backup: its file name, when it was written and its size. */
export function BackupItem({ backup }: { backup: Backup }) {
  return (
    <ItemRow
      start={<IconTile Icon={Archive} />}
      name={backup.name}
      meta={
        <>
          <At ms={backup.modifiedAtMs} /> · {backupSize(backup)}
        </>
      }
    />
  );
}
