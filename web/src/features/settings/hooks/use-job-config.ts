import { useConfigureJob } from "@/features/admin-jobs/data/use-configure-job";
import { jobRefusalFields } from "@/features/admin-jobs/lib/job-refusal";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { refusedWords } from "@/features/settings/lib/refused-words";
import type { Job } from "@/gen/nama/v1/job_pb";

const labels = { enabled: { label: "whether it runs" }, schedule: { label: "its schedule" } };

/**
 * Whether `job` is on and its own schedule, empty for its Plugin's, each saved as it's changed, and
 * what to say if the Server refused one.
 */
export function useJobConfig(job: Job) {
  const { mutateAsync } = useConfigureJob();
  const own = job.schedule === job.defaultSchedule ? "" : job.schedule;
  const { values, refused, edit, dismiss } = useAutoSave(
    { enabled: job.enabled, schedule: own },
    async (wanted) => {
      await mutateAsync({ pluginId: job.pluginId, jobId: job.id, ...wanted });
    },
  );
  const words = refusedWords(refused, labels, refused && jobRefusalFields(refused.failure));
  return { values, words, edit, dismiss };
}
