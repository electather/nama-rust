import { CircleAlert } from "lucide-react";

import { BackUpJobLink } from "@/features/admin-backups/components/back-up-job-link";
import { BackUpNow } from "@/features/admin-backups/components/back-up-now";
import { formatAt } from "@/features/admin-jobs/lib/run-times";
import { StatusCard } from "@/features/settings/components/status-card";
import type { Job, JobRun } from "@/gen/nama/v1/job_pb";
import type { Backup } from "@/gen/nama/v1/server_pb";

/** Says the Back up `job`'s last `run` failed and why, so the Admin sees it without opening it. */
export function FailedBackUp({
  job,
  run,
  newest,
}: {
  job: Job;
  run: JobRun;
  newest: Backup | undefined;
}) {
  return (
    <StatusCard
      setting="schedule"
      Icon={CircleAlert}
      destructive
      title={`The Backup at ${formatAt(run.startedAtMs)} failed`}
      end={<BackUpNow job={job} />}
    >
      <span className="text-xs text-pretty text-destructive">{run.error}</span>
      <span className="text-xs text-muted-foreground">
        {newest && `Last good one ${formatAt(newest.modifiedAtMs)} · `}
        <BackUpJobLink job={job}>See its log</BackUpJobLink>
      </span>
    </StatusCard>
  );
}
