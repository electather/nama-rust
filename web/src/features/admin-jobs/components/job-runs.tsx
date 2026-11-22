import { RunDetail } from "@/features/admin-jobs/components/run-detail";
import { useJobRuns } from "@/features/admin-jobs/data/use-job-runs";
import type { Job } from "@/gen/nama/v1/job_pb";

/** A Job's Runs, newest first, each opening to its detail. */
export function JobRuns({ job }: { job: Job }) {
  const runs = useJobRuns(job);
  if (runs.length === 0) {
    return <p className="py-3.5 text-sm text-muted-foreground">It hasn’t run yet.</p>;
  }
  return (
    <ol aria-label="Runs" className="flex flex-col">
      {runs.map((run) => (
        <RunDetail key={run.id.toString()} run={run} />
      ))}
    </ol>
  );
}
