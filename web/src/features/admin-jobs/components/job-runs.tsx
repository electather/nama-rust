import { RunChart } from "@/features/admin-jobs/components/run-chart";
import { RunDetail } from "@/features/admin-jobs/components/run-detail";
import { RunsHead } from "@/features/admin-jobs/components/runs-head";
import { useJobRuns } from "@/features/admin-jobs/data/use-job-runs";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * A Job's Runs: a chart of how long they took, then a row each, newest first, opening to its
 * detail. The rows reach 8px past the pane's text, so a row's hover is a band with room inside.
 */
export function JobRuns({ job }: { job: Job }) {
  const runs = useJobRuns(job);
  if (runs.length === 0) {
    return <p className="py-3.5 text-sm text-muted-foreground">It hasn’t run yet.</p>;
  }
  const rows = (
    <ol aria-label="Runs" className="flex flex-col">
      {runs.map((run) => (
        <RunDetail key={run.id.toString()} run={run} />
      ))}
    </ol>
  );
  return (
    <>
      <RunChart runs={runs} />
      <div className="-mx-2">
        <RunsHead />
        {rows}
      </div>
    </>
  );
}
