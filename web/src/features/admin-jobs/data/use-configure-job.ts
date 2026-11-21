import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { JobService } from "@/gen/nama/v1/job_pb";

const jobsKey = createConnectQueryKey({
  schema: JobService.method.listJobs,
  cardinality: "finite",
});

/** Turns a Job on or off and sets its schedule, then refetches the Jobs once it's saved. */
export function useConfigureJob() {
  const queryClient = useQueryClient();
  return useMutation(JobService.method.configureJob, {
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: jobsKey });
    },
  });
}
