import { Archive } from "lucide-react";
import { Button } from "nama-ui/components/button";

import { backupSize } from "@/features/admin-backups/lib/backup-facts";
import { At } from "@/features/admin-jobs/components/at";
import { ItemRow } from "@/features/settings/components/item-row";
import type { Backup } from "@/gen/nama/v1/server_pb";
import { IconTile } from "@/shared/components/icon-tile";

/** One Backup: its file name, when it was written and its size, and Restore, which opens `steps`. */
export function BackupItem({
  backup,
  steps,
  open,
  onToggle,
}: {
  backup: Backup;
  /** The id of how to restore it, which shows under it while `open`. */
  steps: string;
  open: boolean;
  onToggle: () => void;
}) {
  const action = open ? "Close" : "Restore";
  return (
    <ItemRow
      beside
      start={<IconTile Icon={Archive} />}
      name={backup.name}
      meta={
        <>
          <At ms={backup.modifiedAtMs} /> · {backupSize(backup)}
        </>
      }
    >
      <Button
        variant="ghost"
        aria-label={`${action} ${backup.name}`}
        aria-expanded={open}
        aria-controls={steps}
        onClick={onToggle}
      >
        {action}
      </Button>
    </ItemRow>
  );
}
