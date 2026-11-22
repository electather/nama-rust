import { useSearch } from "@tanstack/react-router";

import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { jobKey } from "@/shared/lib/job-key";

/** The Job `?job=` opens in the Jobs section: none for an id no Job has, as the list shows then. */
export function useOpenJob() {
  const asked = useSearch({ strict: false, select: ({ job }) => job });
  return useJobs().find((job) => jobKey(job.pluginId, job.id) === asked);
}
