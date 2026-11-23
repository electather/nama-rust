import { PowerOff } from "lucide-react";

import { dayAndTime, relative } from "@/features/admin-jobs/lib/run-times";
import type { Job } from "@/gen/nama/v1/job_pb";

/** When `job` next runs, its day and time on one line, how long from now on hover; or that it's off, or has no schedule. */
export function NextRun({ job }: { job: Job }) {
  const [next] = job.nextRunsAtMs;
  if (!job.enabled) {
    return (
      <span className="inline-flex items-center gap-1.5 text-muted-foreground">
        <PowerOff aria-hidden strokeWidth={1.5} className="size-4" />
        Off
      </span>
    );
  }
  if (next === undefined) {
    return <span className="text-muted-foreground">When an Admin runs it</span>;
  }
  return (
    <time
      dateTime={new Date(Number(next)).toISOString()}
      title={relative(next)}
      className="whitespace-nowrap"
    >
      {dayAndTime(next)}
    </time>
  );
}
