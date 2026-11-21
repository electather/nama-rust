import type { Transport } from "@connectrpc/connect";
import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";

import { JobService } from "@/gen/nama/v1/job_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Jobs of every enabled Plugin, with their next and last Runs. */
export function jobsQuery(transport: Transport) {
  return createQueryOptions(JobService.method.listJobs, {}, { transport });
}

/** The Jobs `jobsQuery` loads. */
export function useJobs() {
  return useSuspenseQuery(JobService.method.listJobs, {}, { staleTime: loaderStaleTime }).data.jobs;
}
