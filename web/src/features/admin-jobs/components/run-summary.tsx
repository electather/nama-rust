import { ChevronRight } from "lucide-react";

import { OutcomeIcon } from "@/features/admin-jobs/components/outcome-icon";
import { TOOK_COLUMN } from "@/features/admin-jobs/lib/run-columns";
import { dayAndTime, duration, formatAt } from "@/features/admin-jobs/lib/run-times";
import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** How long `run` took, or why that isn't known. */
function took(run: JobRun) {
  if (run.endedAtMs !== undefined) {
    return duration(run.startedAtMs, run.endedAtMs);
  }
  return run.outcome === JobRunOutcome.RUNNING ? "So far" : "Not known";
}

/** What a closed Run shows: how it ended, as its icon, when it started and how long it took. */
export function RunSummary({ run }: { run: JobRun }) {
  const at = new Date(Number(run.startedAtMs));
  return (
    <summary className="flex cursor-pointer list-none items-center gap-3 rounded-sm p-2 text-sm transition-colors outline-none hover:bg-muted/50 focus-visible:ring-3 focus-visible:ring-ring/50 motion-reduce:transition-none [&::-webkit-details-marker]:hidden">
      <OutcomeIcon outcome={run.outcome} />
      <time
        dateTime={at.toISOString()}
        title={formatAt(run.startedAtMs)}
        className="min-w-0 flex-1 tabular-nums"
      >
        {dayAndTime(run.startedAtMs)}
      </time>
      <span className={`text-muted-foreground tabular-nums ${TOOK_COLUMN}`}>{took(run)}</span>
      <ChevronRight
        aria-hidden
        className="size-4 shrink-0 transition-transform duration-150 ease-out-expo group-open/run:rotate-90 motion-reduce:transition-none"
      />
    </summary>
  );
}
