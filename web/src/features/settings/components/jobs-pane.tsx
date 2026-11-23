import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { FailedJobs } from "@/features/settings/components/failed-jobs";
import { JobDetail } from "@/features/settings/components/job-detail";
import { JobTable } from "@/features/settings/components/job-table";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { useOpenJob } from "@/features/settings/hooks/use-open-job";

/**
 * The enabled Plugins' Jobs, for an Admin: those whose last Run failed, then a table of them by
 * Plugin; or in its place the one `?job=<pluginId>:<jobId>` names, with its settings and Runs.
 */
export function JobsPane() {
  const jobs = useJobs();
  const open = useOpenJob();
  if (open) {
    return <JobDetail job={open} />;
  }
  const listed = (
    <div className="flex flex-col gap-4">
      <FailedJobs jobs={jobs} />
      <JobTable jobs={jobs} />
    </div>
  );
  return (
    <PaneFrame section="jobs">
      {jobs.length === 0 ? (
        <p className="py-3.5 text-sm text-muted-foreground">No enabled Plugin has Jobs.</p>
      ) : (
        listed
      )}
    </PaneFrame>
  );
}
