import { BackUpJobLink } from "@/features/admin-backups/components/back-up-job-link";
import { formatAt } from "@/features/admin-jobs/lib/run-times";
import type { Job } from "@/gen/nama/v1/job_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";

/** When the Back up `job` runs by itself and next will, with a link to its Job in Settings, to change when. */
export function BackUpSchedule({ job }: { job: Job }) {
  const [next] = job.nextRunsAtMs;
  return (
    <span className="text-xs text-pretty text-muted-foreground">
      {job.enabled ? scheduleWords(job.schedule) : "Turned off: only when you back up now"}
      {job.enabled && next !== undefined && ` · Next ${formatAt(next)}`} ·{" "}
      <BackUpJobLink job={job}>Change</BackUpJobLink>
    </span>
  );
}
