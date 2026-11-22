import { BackUpSchedule } from "@/features/admin-backups/components/back-up-schedule";
import { LastBackUpFailed } from "@/features/admin-backups/components/last-back-up-failed";
import { UpkeepDisabled } from "@/features/admin-backups/components/upkeep-disabled";
import type { Job } from "@/gen/nama/v1/job_pb";

/** When the Back up `job` runs by itself and whether its last Run failed, or that Upkeep is off. */
export function BackUpWhen({ job }: { job: Job | undefined }) {
  if (job === undefined) {
    return <UpkeepDisabled />;
  }
  return (
    <div className="flex flex-col gap-2 py-3.5">
      <BackUpSchedule job={job} />
      <LastBackUpFailed lastRun={job.lastRun} />
    </div>
  );
}
