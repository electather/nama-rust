import { useRouteContext } from "@tanstack/react-router";

import { PluginJobItem } from "@/features/admin-plugins/components/plugin-job-item";
import type { PluginJob, PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * For an Admin, the Jobs the Plugin declares, enabled or not, each linking to it in Settings' Jobs
 * section, which only Admins open, while the Plugin is enabled. A Plugin with none has no section.
 */
export function PluginJobs({ summary, jobs }: { summary: PluginSummary; jobs: PluginJob[] }) {
  const { signedIn } = useRouteContext({ from: "/_member" });
  if (jobs.length === 0 || !may(signedIn.user, Permission.ADMIN)) {
    return null;
  }
  return (
    <section aria-labelledby="jobs" className="mt-10">
      <h2 id="jobs" className="font-medium">
        Jobs
      </h2>
      <ul className="mt-3 flex flex-col gap-3">
        {jobs.map((job) => (
          <PluginJobItem
            key={job.id}
            job={job}
            // Jobs lists only enabled Plugins' Jobs, so a disabled one's would lead nowhere.
            pluginId={summary.enabled ? summary.id : undefined}
          />
        ))}
      </ul>
    </section>
  );
}
