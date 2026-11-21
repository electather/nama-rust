import { BackUpSchedule } from "@/features/admin-backups/components/back-up-schedule";
import { LastBackUpFailed } from "@/features/admin-backups/components/last-back-up-failed";
import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { isRunning } from "@/features/admin-jobs/lib/outcomes";
import { runRefused } from "@/features/admin-jobs/lib/run-refused";
import type { Job } from "@/gen/nama/v1/job_pb";
import { Button } from "@/shared/ui/button";

/** Runs the Back up Job now, the page's one primary action, with when it runs by itself. */
export function BackUpNow({ job }: { job: Job }) {
  const run = useRunJob();
  const running = isRunning(job.lastRun);
  return (
    <div className="mt-6 flex flex-col items-start gap-3">
      <Button
        disabled={run.isPending || running}
        onClick={() => {
          run.mutate({ pluginId: job.pluginId, jobId: job.id });
        }}
      >
        {running ? "Backing up…" : "Back up now"}
      </Button>
      <p role="alert" className="text-sm text-destructive empty:hidden">
        {run.isError ? runRefused(run.error) : undefined}
      </p>
      <LastBackUpFailed lastRun={job.lastRun} />
      <BackUpSchedule job={job} />
    </div>
  );
}
