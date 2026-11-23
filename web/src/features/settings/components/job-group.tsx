import { useId } from "react";

import type { PluginJobs } from "@/features/admin-jobs/lib/by-plugin";
import { JobItem } from "@/features/settings/components/job-item";

/** One Plugin's Jobs in the table, under its name. */
export function JobGroup({ group }: { group: PluginJobs }) {
  const id = useId();
  const name = (
    <th
      id={id}
      colSpan={4}
      scope="colgroup"
      className="px-2 pt-4 pb-1 text-start text-xs font-normal text-muted-foreground"
    >
      {group.pluginName}
    </th>
  );
  return (
    <tbody aria-labelledby={id}>
      <tr>{name}</tr>
      {group.jobs.map((job) => (
        <JobItem key={job.id} job={job} />
      ))}
    </tbody>
  );
}
