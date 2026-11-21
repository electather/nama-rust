import { useSuspenseQuery } from "@connectrpc/connect-query";

import { isRunning, RUNNING_POLL_MS } from "@/features/admin-jobs/lib/outcomes";
import { type Job, JobService } from "@/gen/nama/v1/job_pb";

/** The Runs of `job`, newest first, looked at again while the newest is going. */
export function useJobRuns(job: Job) {
  return useSuspenseQuery(
    JobService.method.listJobRuns,
    { pluginId: job.pluginId, jobId: job.id },
    {
      refetchInterval: (query) => (isRunning(query.state.data?.runs[0]) ? RUNNING_POLL_MS : false),
    },
  ).data.runs;
}
