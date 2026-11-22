import { OutcomeIcon } from "@/features/admin-jobs/components/outcome-icon";
import { TimeAgo } from "@/features/admin-jobs/components/time-ago";
import type { Job } from "@/gen/nama/v1/job_pb";

/** How `job`'s last Run went, as its icon, beside when it started; or that it never ran. */
export function LastRun({ job: { lastRun } }: { job: Job }) {
  if (!lastRun) {
    return <span className="text-muted-foreground">Never run</span>;
  }
  return (
    <span className="flex items-center gap-2 whitespace-nowrap">
      <OutcomeIcon outcome={lastRun.outcome} />
      <TimeAgo ms={lastRun.startedAtMs} />
    </span>
  );
}
