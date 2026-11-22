import { Play } from "lucide-react";

import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { isRunning } from "@/features/admin-jobs/lib/outcomes";
import type { Job } from "@/gen/nama/v1/job_pb";
import { IconAction } from "@/shared/components/icon-action";

/** Run now for the open Job `job`, off while a Run of it is going. */
export function RunOpenJob({ job }: { job: Job }) {
  const run = useRunJob(job);
  const going = isRunning(job.lastRun);
  return (
    <IconAction
      label={going ? "It’s running" : "Run now"}
      Icon={Play}
      aria-label={`Run ${job.name} now`}
      disabled={going || run.isPending}
      onClick={() => {
        run.mutate({ pluginId: job.pluginId, jobId: job.id });
      }}
    />
  );
}
