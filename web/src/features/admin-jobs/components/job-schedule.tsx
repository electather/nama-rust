import { NextRuns } from "@/features/admin-jobs/components/next-runs";
import type { Job } from "@/gen/nama/v1/job_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";

/** The top of a Job's sheet: its name, what it does, its schedule and when it next runs. */
export function JobSchedule({ job, headingId }: { job: Job; headingId: string }) {
  return (
    <header className="flex flex-col gap-2 pe-8">
      <h2 id={headingId} className="text-lg font-semibold text-balance">
        {job.name}
      </h2>
      {job.description !== "" && <p className="text-pretty">{job.description}</p>}
      <p className="text-sm text-muted-foreground">{scheduleWords(job.schedule)}</p>
      <NextRuns times={job.nextRunsAtMs} />
    </header>
  );
}
