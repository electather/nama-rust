import { JobRunsLoader } from "@/features/admin-jobs/components/job-runs-loader";
import { JobSchedule } from "@/features/admin-jobs/components/job-schedule";
import type { Job } from "@/gen/nama/v1/job_pb";
import { CloseButton } from "@/shared/components/confirm-dialog/close-button";

/** What a Job's sheet holds: its schedule and its Runs, with a ✕ that closes it. */
export function JobSheetBody({
  job,
  headingId,
  close,
}: {
  job: Job;
  headingId: string;
  close: () => void;
}) {
  return (
    <div className="flex flex-col gap-6 p-6">
      <CloseButton disabled={false} onClose={close} />
      <JobSchedule job={job} headingId={headingId} />
      <JobRunsLoader job={job} />
    </div>
  );
}
