import { LoaderCircle } from "lucide-react";

import { BackUpNow } from "@/features/admin-backups/components/back-up-now";
import { formatAt } from "@/features/admin-jobs/lib/run-times";
import { StatusCard } from "@/features/settings/components/status-card";
import type { Job, JobRun } from "@/gen/nama/v1/job_pb";

/** Says the Back up `job` is writing a Backup, in the `run` it started. */
export function BackingUp({ job, run }: { job: Job; run: JobRun }) {
  return (
    <StatusCard
      setting="schedule"
      Icon={LoaderCircle}
      title="Backing up now…"
      end={<BackUpNow job={job} />}
    >
      <span className="text-xs text-muted-foreground">Started {formatAt(run.startedAtMs)}</span>
    </StatusCard>
  );
}
