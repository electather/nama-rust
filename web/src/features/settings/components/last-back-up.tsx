import { Archive, CircleCheck } from "lucide-react";

import { BackUpNow } from "@/features/admin-backups/components/back-up-now";
import { BackUpSchedule } from "@/features/admin-backups/components/back-up-schedule";
import { backupSize } from "@/features/admin-backups/lib/backup-facts";
import { formatAt } from "@/features/admin-jobs/lib/run-times";
import { StatusCard } from "@/features/settings/components/status-card";
import type { Job } from "@/gen/nama/v1/job_pb";
import type { Backup } from "@/gen/nama/v1/server_pb";

/** The `newest` Backup, or that there's none yet, and when the Back up `job` writes the next. */
export function LastBackUp({ job, newest }: { job: Job; newest: Backup | undefined }) {
  return (
    <StatusCard
      setting="schedule"
      Icon={newest ? CircleCheck : Archive}
      title={
        newest
          ? `Last Backup ${formatAt(newest.modifiedAtMs)} · ${backupSize(newest)}`
          : "No Backups yet"
      }
      end={<BackUpNow job={job} />}
    >
      <BackUpSchedule job={job} />
    </StatusCard>
  );
}
