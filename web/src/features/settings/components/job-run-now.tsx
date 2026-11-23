import { RunOpenJob } from "@/features/settings/components/run-open-job";
import { useOpenJob } from "@/features/settings/hooks/use-open-job";

/** Run now, beside Settings' close while a Job is open. */
export function JobRunNow() {
  const job = useOpenJob();
  return job ? <RunOpenJob job={job} /> : null;
}
