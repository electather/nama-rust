import { RunBars } from "@/features/admin-jobs/components/run-bars";
import { endedOldestFirst, failedOf } from "@/features/admin-jobs/lib/run-chart";
import { dayAndTime } from "@/features/admin-jobs/lib/run-times";
import type { JobRun } from "@/gen/nama/v1/job_pb";

/**
 * How long a Job's last 20 Runs that ended took, oldest first, failed ones red, with a dashed line
 * at how long it usually takes, between the first and last's dates; nothing until two have ended.
 */
export function RunChart({ runs }: { runs: JobRun[] }) {
  const ended = endedOldestFirst(runs);
  if (ended.length < 2) {
    return null;
  }
  const failed = failedOf(ended);
  const ends = ended
    .filter((_, index) => index === 0 || index === ended.length - 1)
    .map((run) => <span key={run.id.toString()}>{dayAndTime(run.startedAtMs)}</span>);
  return (
    <figure className="flex flex-col gap-1.5 pt-2 pb-3 text-xs text-muted-foreground">
      <figcaption className="text-sm text-foreground">
        Last {ended.length} Runs
        {failed > 0 && <span className="text-destructive"> · {failed} failed</span>}
      </figcaption>
      <RunBars ended={ended} />
      <span className="flex justify-between gap-4 tabular-nums">{ends}</span>
    </figure>
  );
}
