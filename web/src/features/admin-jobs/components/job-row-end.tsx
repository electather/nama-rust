import { PowerOff } from "lucide-react";

import { ConfigureJob } from "@/features/admin-jobs/components/configure-job";
import { RunNowButton } from "@/features/admin-jobs/components/run-now-button";
import { RunOutcome } from "@/features/admin-jobs/components/run-outcome";
import type { Job } from "@/gen/nama/v1/job_pb";
import { StateLabel } from "@/shared/components/state-label";

/** The end of a Job's row: whether it's off, its last Run's outcome, Run now and Configure. */
export function JobRowEnd({ job }: { job: Job }) {
  return (
    <div className="flex items-center gap-2">
      {!job.enabled && <StateLabel label="Off" Icon={PowerOff} />}
      {job.lastRun && <RunOutcome outcome={job.lastRun.outcome} />}
      <RunNowButton job={job} />
      <ConfigureJob job={job} />
    </div>
  );
}
