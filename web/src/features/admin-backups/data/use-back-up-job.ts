import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";

import { backupsKey } from "@/features/admin-backups/data/use-backups";
import { BACK_UP_JOB, UPKEEP } from "@/features/admin-backups/lib/back-up-job";
import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { isRunning } from "@/features/admin-jobs/lib/outcomes";

/**
 * Upkeep's Back up Job, looked at again while it runs, or `undefined` while Upkeep is disabled.
 * Once a Run ends, the Backups are listed again, so the one it wrote shows.
 */
export function useBackUpJob() {
  const queryClient = useQueryClient();
  const job = useJobs().find(({ pluginId, id }) => pluginId === UPKEEP && id === BACK_UP_JOB);
  const runId = job?.lastRun?.id;
  const running = isRunning(job?.lastRun);
  // A Backup of a small Server can end before the Jobs are first fetched again, so a new Run
  // that has already ended counts as one ending.
  const seen = useRef({ runId, running });
  useEffect(() => {
    if (!running && (seen.current.running || seen.current.runId !== runId)) {
      void queryClient.invalidateQueries({ queryKey: backupsKey });
    }
    seen.current = { runId, running };
  }, [runId, running, queryClient]);
  return job;
}
