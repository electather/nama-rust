import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { type Job, JobService } from "@/gen/nama/v1/job_pb";

/** What a Run now of `job` is kept under, so its Job's view can say why one didn't start. */
export const runKey = (job: Job) => ["run-job", job.pluginId, job.id];

/** Starts a Run of `job` now, then refetches the Jobs and their Runs, which list it, either way. */
export function useRunJob(job: Job) {
  const queryClient = useQueryClient();
  return useMutation(JobService.method.runJob, {
    mutationKey: runKey(job),
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({ schema: JobService, cardinality: "finite" }),
      });
    },
  });
}
