import { LastRun } from "@/features/admin-jobs/components/last-run";
import { NextRun } from "@/features/admin-jobs/components/next-run";
import { RunNowButton } from "@/features/admin-jobs/components/run-now-button";
import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { runRefusedBy } from "@/features/admin-jobs/lib/run-refused";
import { JobName } from "@/features/settings/components/job-name";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * One Job's row of the table: its name and schedule, how its last Run went, when it next runs and
 * Run now. All of the row but Run now opens it.
 */
export function JobItem({ job }: { job: Job }) {
  const run = useRunJob(job);
  return (
    <tr className="relative border-t transition-colors hover:bg-muted/50 motion-reduce:transition-none">
      <td className="py-2 ps-2 pe-3">
        <JobName job={job} failed={runRefusedBy(run)} />
      </td>
      <td className="p-2">
        <LastRun job={job} />
      </td>
      <td className="p-2 max-md:hidden">
        <NextRun job={job} />
      </td>
      <td className="relative py-2 pe-2 text-end">
        <RunNowButton job={job} run={run} />
      </td>
    </tr>
  );
}
