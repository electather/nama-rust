import { useJobs } from "@/features/admin-jobs/data/use-jobs";
import { byPlugin } from "@/features/admin-jobs/lib/by-plugin";
import { JobDetail } from "@/features/settings/components/job-detail";
import { JobGroup } from "@/features/settings/components/job-group";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { useOpenJob } from "@/features/settings/hooks/use-open-job";

/**
 * The enabled Plugins' Jobs, for an Admin: the list of them by Plugin, or in its place the one
 * `?job=<pluginId>:<jobId>` names, with its settings and Runs.
 */
export function JobsPane() {
  const jobs = useJobs();
  const open = useOpenJob();
  if (open) {
    return <JobDetail job={open} />;
  }
  const groups = byPlugin(jobs).map((group) => <JobGroup key={group.pluginId} group={group} />);
  return (
    <PaneFrame section="jobs">
      {jobs.length === 0 ? (
        <p className="py-3.5 text-sm text-muted-foreground">No enabled Plugin has Jobs.</p>
      ) : (
        groups
      )}
    </PaneFrame>
  );
}
