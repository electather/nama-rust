import { Play } from "lucide-react";
import { Button } from "nama-ui/components/button";

import type { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Starts a Run of `job` now, scheduled or not, through `run`, which says why if it didn't start. */
export function RunNowButton({ job, run }: { job: Job; run: ReturnType<typeof useRunJob> }) {
  return (
    <Button
      variant="ghost"
      size="icon-sm"
      aria-label={`Run ${job.name} now`}
      title="Run now"
      disabled={run.isPending}
      onClick={() => {
        run.mutate({ pluginId: job.pluginId, jobId: job.id });
      }}
    >
      <Play aria-hidden strokeWidth={1.5} />
    </Button>
  );
}
