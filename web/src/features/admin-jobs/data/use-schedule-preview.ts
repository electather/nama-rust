import { useQuery } from "@connectrpc/connect-query";
import { keepPreviousData, skipToken } from "@tanstack/react-query";

import { JobService } from "@/gen/nama/v1/job_pb";

/**
 * When `schedule` would next fire, from the Server, which reads it in its own time zone, and why it
 * can't if it can't; nothing asked while `skip`. Keeps the last answer while the next loads, so the
 * preview doesn't jump.
 */
export function useSchedulePreview(schedule: string, skip: boolean) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the preview keeps its last answer while the next schedule picked loads, rather than the picker falling back to a skeleton
  const { data, error } = useQuery(
    JobService.method.previewJobSchedule,
    skip ? skipToken : { schedule },
    { placeholderData: keepPreviousData, retry: false },
  );
  return { next: data?.nextRunsAtMs, error };
}
