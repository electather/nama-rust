import { useQuery } from "@connectrpc/connect-query";
import { keepPreviousData } from "@tanstack/react-query";

import { jobRefusal } from "@/features/admin-jobs/lib/job-refusal";
import { JobService } from "@/gen/nama/v1/job_pb";

/** When `schedule` would next fire, or why the Server can't read it, asked again as it changes. */
export function useSchedulePreview(schedule: string) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the field must stay typeable while the Server reads each change
  const { data, error } = useQuery(
    JobService.method.previewJobSchedule,
    { schedule },
    { retry: false, placeholderData: keepPreviousData },
  );
  return {
    // A schedule the Server refused has no data: the last one's is kept only while it's asked.
    nextRunsAtMs: data?.nextRunsAtMs ?? [],
    error: error ? jobRefusal(error, "check the schedule") : undefined,
  };
}
