import { Fragment, useId, useState } from "react";

import { BackupItem } from "@/features/settings/components/backup-item";
import { RestoreSteps } from "@/features/settings/components/restore-steps";
import type { Backup } from "@/gen/nama/v1/server_pb";

/**
 * The Backups in the Backup `folder`, newest first, each with how to restore it under it once
 * asked, one at a time, or a line saying there are none yet.
 */
export function BackupItems({ backups, folder }: { backups: Backup[]; folder: string }) {
  const id = useId();
  const [open, setOpen] = useState<string>();
  if (backups.length === 0) {
    return <p className="py-1.5 text-sm text-muted-foreground">None yet.</p>;
  }
  return (
    <ul aria-label="Backups" className="-mb-1.5 flex flex-col">
      {backups.map((backup, index) => {
        const { name } = backup;
        const shown = open === name;
        const steps = `${id}-${index}`;
        return (
          <Fragment key={name}>
            <BackupItem
              backup={backup}
              steps={steps}
              open={shown}
              onToggle={() => {
                setOpen(shown ? undefined : name);
              }}
            />
            {shown && <RestoreSteps id={steps} folder={folder} name={name} />}
          </Fragment>
        );
      })}
    </ul>
  );
}
