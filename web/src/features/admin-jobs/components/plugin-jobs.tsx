import { Link } from "@tanstack/react-router";

import { JobRow } from "@/features/admin-jobs/components/job-row";
import type { PluginJobs as Group } from "@/features/admin-jobs/lib/by-plugin";
import type { Job } from "@/gen/nama/v1/job_pb";
import { AdminList } from "@/shared/components/admin-list/admin-list";

/** One Plugin's Jobs, under its name, which links to its page. */
export function PluginJobs({ group, onOpen }: { group: Group; onOpen: (job: Job) => void }) {
  return (
    <section className="mt-10">
      <h2 className="text-lg font-semibold">
        <Link
          to="/admin/plugins/$pluginId"
          params={{ pluginId: group.pluginId }}
          className="rounded-sm outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {group.pluginName}
        </Link>
      </h2>
      <AdminList>
        {group.jobs.map((job) => (
          <JobRow key={job.id} job={job} onOpen={onOpen} />
        ))}
      </AdminList>
    </section>
  );
}
