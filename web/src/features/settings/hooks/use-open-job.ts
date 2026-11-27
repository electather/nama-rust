import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";

/** The Job `?job=` opens in the Jobs section: none for an id no Job has, as the list shows then. */
export function useOpenJob() {
  const { detail } = useSettingsAddress();
  return useJobs().find(
    (job) => detail?.kind === "job" && job.pluginId === detail.pluginId && job.id === detail.jobId,
  );
}
