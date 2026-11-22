import type { PluginJobs } from "@/features/admin-jobs/lib/by-plugin";
import { JobItem } from "@/features/settings/components/job-item";
import { SettingGroup } from "@/features/settings/components/setting-group";

/** One Plugin's Jobs, under its name. */
export function JobGroup({ group }: { group: PluginJobs }) {
  return (
    <SettingGroup title={group.pluginName}>
      <ul aria-label={group.pluginName} className="-mb-1.5 flex flex-col">
        {group.jobs.map((job) => (
          <JobItem key={job.id} job={job} />
        ))}
      </ul>
    </SettingGroup>
  );
}
