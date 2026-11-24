import { useTransport } from "@connectrpc/connect-query";
import { useMutationState } from "@tanstack/react-query";

import { runKey } from "@/features/admin-jobs/data/use-run-job";
import { runRefused } from "@/features/admin-jobs/lib/run-refused";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Why the last Run now of `job`, from wherever it was pressed, didn't start, or nothing. */
export function useRunRefusal(job: Job) {
  const transport = useTransport();
  const tries = useMutationState({
    filters: { mutationKey: runKey(transport, job) },
    select: (each) => each.state,
  });
  const last = tries.at(-1);
  return last?.status === "error" ? runRefused(last.error) : undefined;
}
