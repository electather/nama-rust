import { createConnectQueryKey, useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";

import { JobService } from "@/gen/nama/v1/job_pb";

/** Starts a Run of a Job now, then refetches the Jobs and their Runs, which list it, either way. */
export function useRunJob() {
  const queryClient = useQueryClient();
  return useMutation(JobService.method.runJob, {
    onSettled: async () => {
      await queryClient.invalidateQueries({
        queryKey: createConnectQueryKey({ schema: JobService, cardinality: "finite" }),
      });
    },
  });
}
