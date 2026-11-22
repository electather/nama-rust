import { Button } from "nama-ui/components/button";

import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { runRefused } from "@/features/admin-jobs/lib/run-refused";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Starts a Run of `job` now, scheduled or not, saying why if it didn't start. */
export function RunNowButton({ job }: { job: Job }) {
  const run = useRunJob();
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`Run ${job.name} now`}
        disabled={run.isPending}
        onClick={() => {
          run.mutate({ pluginId: job.pluginId, jobId: job.id });
        }}
      >
        Run now
      </Button>
      {run.isError && (
        <p role="alert" className="basis-full text-sm text-destructive">
          {runRefused(run.error)}
        </p>
      )}
    </>
  );
}
