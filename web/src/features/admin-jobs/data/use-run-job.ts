import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { type Job, JobService } from "@/gen/nama/v1/job_pb";
import { serverKey } from "@/shared/hooks/server-key";

/** What a Run now of `job` on the Server `transport` talks to is kept under, so its Job's view can say why one didn't start. */
export const runKey = (transport: Transport, job: Job) => [
  "run-job",
  serverKey(transport),
  job.pluginId,
  job.id,
];

/** Starts a Run of `job` now, then refetches the Jobs and their Runs, which list it, either way. */
export function useRunJob(job: Job) {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(JobService.method.runJob, {
    mutationKey: runKey(transport, job),
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({ schema: JobService, transport, cardinality: "finite" }),
      });
    },
  });
}
