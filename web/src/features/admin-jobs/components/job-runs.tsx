import { useId } from "react";

import { RunDetail } from "@/features/admin-jobs/components/run-detail";
import { useJobRuns } from "@/features/admin-jobs/data/use-job-runs";
import type { Job } from "@/gen/nama/v1/job_pb";

/** A Job's Runs, newest first, each opening to its detail. */
export function JobRuns({ job }: { job: Job }) {
  const runs = useJobRuns(job);
  const id = useId();
  return (
    <section aria-labelledby={id}>
      <h3 id={id} className="font-medium">
        Runs
      </h3>
      {runs.length === 0 ? (
        <p className="mt-2 text-sm text-muted-foreground">It hasn’t run yet.</p>
      ) : (
        <ol className="mt-2 flex flex-col">
          {runs.map((run) => (
            <RunDetail key={run.id.toString()} run={run} />
          ))}
        </ol>
      )}
    </section>
  );
}
