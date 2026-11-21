import { Link } from "@tanstack/react-router";

import type { PluginJob } from "@/gen/nama/v1/plugin_pb";
import { jobAnchor } from "@/shared/lib/job-anchor";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { textLink } from "@/shared/lib/text-link-classes";

/** One of a Plugin's Jobs: its name, linking to its row on the Jobs page, and its schedule. */
export function PluginJobItem({ pluginId, job }: { pluginId: string; job: PluginJob }) {
  return (
    <li className="flex flex-col items-start">
      <Link to="/admin/jobs" hash={jobAnchor(pluginId, job.id)} className={textLink}>
        {job.name}
      </Link>
      <span className="text-sm text-muted-foreground">{scheduleWords(job.schedule)}</span>
    </li>
  );
}
