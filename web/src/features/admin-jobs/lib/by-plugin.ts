import type { Job } from "@/gen/nama/v1/job_pb";
import { pluginName } from "@/shared/lib/plugins";

/** The Jobs one Plugin declares. */
export interface PluginJobs {
  pluginId: string;
  pluginName: string;
  jobs: Job[];
}

/** `jobs` grouped by the Plugin declaring them, in the order they come. */
export function byPlugin(jobs: Job[]): PluginJobs[] {
  const groups = new Map<string, PluginJobs>();
  for (const job of jobs) {
    const group = groups.get(job.pluginId) ?? {
      pluginId: job.pluginId,
      pluginName: pluginName({ id: job.pluginId, name: job.pluginName }),
      jobs: [],
    };
    group.jobs.push(job);
    groups.set(job.pluginId, group);
  }
  return [...groups.values()];
}
