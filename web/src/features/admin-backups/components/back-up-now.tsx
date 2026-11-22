import { Button } from "nama-ui/components/button";

import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { isRunning } from "@/features/admin-jobs/lib/outcomes";
import { runRefused } from "@/features/admin-jobs/lib/run-refused";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Runs the Back up Job now, with why the Server wouldn't start it, if it wouldn't. */
export function BackUpNow({ job }: { job: Job }) {
  const run = useRunJob();
  const running = isRunning(job.lastRun);
  return (
    <div className="flex flex-col items-end gap-2">
      <Button
        disabled={run.isPending || running}
        onClick={() => {
          run.mutate({ pluginId: job.pluginId, jobId: job.id });
        }}
      >
        {running ? "Backing up…" : "Back up now"}
      </Button>
      <p role="alert" className="text-sm text-pretty text-destructive empty:hidden">
        {run.isError ? runRefused(run.error) : undefined}
      </p>
    </div>
  );
}
