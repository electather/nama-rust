import { useSuspenseQuery } from "@connectrpc/connect-query";

import { isRunning, RUNNING_POLL_MS } from "@/features/admin-jobs/lib/outcomes";
import { JobService } from "@/gen/nama/v1/job_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Jobs of every enabled Plugin, with their next and last Runs, looked at again while any has a Run going. */
export function useJobs() {
  return useSuspenseQuery(
    JobService.method.listJobs,
    {},
    {
      staleTime: loaderStaleTime,
      refetchInterval: (query) =>
        query.state.data?.jobs.some((job) => isRunning(job.lastRun)) === true
          ? RUNNING_POLL_MS
          : false,
    },
  ).data.jobs;
}
