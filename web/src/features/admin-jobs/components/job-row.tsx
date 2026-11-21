import { RunOutcome } from "@/features/admin-jobs/components/run-outcome";
import { nextAndLast } from "@/features/admin-jobs/lib/run-times";
import type { Job } from "@/gen/nama/v1/job_pb";
import { AdminRow } from "@/shared/components/admin-list/admin-row";
import { jobAnchor } from "@/shared/lib/job-anchor";
import { scheduleWords } from "@/shared/lib/schedule-words";

/** One Job on the list: its name, which opens its sheet, its schedule, and its next and last Runs. */
export function JobRow({ job, onOpen }: { job: Job; onOpen: (job: Job) => void }) {
  return (
    <AdminRow end={job.lastRun && <RunOutcome outcome={job.lastRun.outcome} />}>
      <button
        type="button"
        id={jobAnchor(job.pluginId, job.id)}
        aria-haspopup="dialog"
        onClick={() => {
          onOpen(job);
        }}
        className="truncate rounded-sm text-start font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        {job.name}
      </button>
      <span className="text-sm">{scheduleWords(job.schedule)}</span>
      <span className="text-sm text-muted-foreground">{nextAndLast(job)}</span>
    </AdminRow>
  );
}
