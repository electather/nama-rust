import { PowerOff } from "lucide-react";

import { RunOutcome } from "@/features/admin-jobs/components/run-outcome";
import type { Job } from "@/gen/nama/v1/job_pb";
import { StateLabel } from "@/shared/components/state-label";

/** Whether `job` is off, and how its last Run ended. */
export function JobState({ job }: { job: Job }) {
  return (
    <>
      {!job.enabled && <StateLabel label="Off" Icon={PowerOff} />}
      {job.lastRun && <RunOutcome outcome={job.lastRun.outcome} />}
    </>
  );
}
