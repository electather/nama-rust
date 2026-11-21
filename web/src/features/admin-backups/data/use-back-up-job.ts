import { useSuspenseQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { backupsKey } from "@/features/admin-backups/data/use-backups";
import { BACK_UP_JOB, UPKEEP } from "@/features/admin-backups/lib/back-up-job";
import { isRunning, RUNNING_POLL_MS } from "@/features/admin-jobs/lib/outcomes";
import { JobService } from "@/gen/nama/v1/job_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/**
 * Upkeep's Back up Job, looked at again while it runs, or `undefined` while Upkeep is disabled.
 * Once a Run ends, the Backups are listed again, so the one it wrote shows.
 */
export function useBackUpJob() {
  const queryClient = useQueryClient();
  const job = useSuspenseQuery(
    JobService.method.listJobs,
    {},
    {
      staleTime: loaderStaleTime,
      refetchInterval: (query) =>
        isRunning(query.state.data?.jobs.find(isBackUp)?.lastRun) ? RUNNING_POLL_MS : false,
    },
  ).data.jobs.find(isBackUp);
  const running = isRunning(job?.lastRun);
  const wasRunning = useRef(running);
  useEffect(() => {
    if (wasRunning.current && !running) {
      void queryClient.invalidateQueries({ queryKey: backupsKey });
    }
    wasRunning.current = running;
  }, [running, queryClient]);
  return job;
}

function isBackUp(job: { pluginId: string; id: string }) {
  return job.pluginId === UPKEEP && job.id === BACK_UP_JOB;
}
