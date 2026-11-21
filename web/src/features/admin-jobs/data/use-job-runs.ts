import { useSuspenseQuery } from "@connectrpc/connect-query";

import { type Job, JobService } from "@/gen/nama/v1/job_pb";

/** The Runs of `job`, newest first. */
export function useJobRuns(job: Job) {
  return useSuspenseQuery(JobService.method.listJobRuns, {
    pluginId: job.pluginId,
    jobId: job.id,
  }).data.runs;
}
