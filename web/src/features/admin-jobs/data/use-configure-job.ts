import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useMutation, useTransport } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { JobService } from "@/gen/nama/v1/job_pb";

function jobsKey(transport: Transport) {
  return createConnectQueryKey({
    schema: JobService.method.listJobs,
    cardinality: "finite",
    transport,
  });
}

/** Turns a Job on or off and sets its schedule, then refetches the Jobs once it's saved. */
export function useConfigureJob() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMutation(JobService.method.configureJob, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: jobsKey(transport) });
    },
  });
}
