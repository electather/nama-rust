import { useJobRuns } from "@/features/admin-jobs/data/use-job-runs";
import { roughly, usualMs } from "@/features/admin-jobs/lib/run-times";
import type { Job } from "@/gen/nama/v1/job_pb";

/** How long `job` usually takes, from the Runs of it that succeeded, or that it isn't known yet. */
export function UsualTime({ job }: { job: Job }) {
  const ms = usualMs(useJobRuns(job));
  return ms === undefined ? "Not known yet" : roughly(ms);
}
