import { useRouteContext } from "@tanstack/react-router";

import { PluginJobItem } from "@/features/admin-plugins/components/plugin-job-item";
import type { PluginJob } from "@/gen/nama/v1/plugin_pb";
import { Permission } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/shared/lib/permissions";

/**
 * For an Admin, the Jobs the Plugin `pluginId` declares, enabled or not, each linking to it on the
 * Jobs page, which only Admins open. A Plugin with none has no section.
 */
export function PluginJobs({ pluginId, jobs }: { pluginId: string; jobs: PluginJob[] }) {
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
          <PluginJobItem key={job.id} pluginId={pluginId} job={job} />
        ))}
      </ul>
    </section>
  );
}
