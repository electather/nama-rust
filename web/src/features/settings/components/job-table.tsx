import { byPlugin } from "@/features/admin-jobs/lib/by-plugin";
import { JobGroup } from "@/features/settings/components/job-group";
import { JobTableHead } from "@/features/settings/components/job-table-head";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * The Jobs as a table, one group of rows to a Plugin: each Job's name and schedule, how its last
 * Run went, when it runs next and Run now. The rows reach 8px past the pane's text, so a row's
 * hover is one band with room inside it.
 */
export function JobTable({ jobs }: { jobs: Job[] }) {
  const groups = byPlugin(jobs).map((group) => <JobGroup key={group.pluginId} group={group} />);
  return (
    <div className="-mx-2">
      <table className="w-full border-collapse text-sm">
        <caption className="caption-bottom px-2 pt-2.5 text-start text-xs text-muted-foreground">
          Schedules are on the Server’s clock.
        </caption>
        <JobTableHead />
        {groups}
      </table>
    </div>
  );
}
