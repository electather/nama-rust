import { Link } from "@tanstack/react-router";

import type { PluginJob } from "@/gen/nama/v1/plugin_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * One of a Plugin's Jobs: its name, linking to `anchor`, its row on the Jobs page, where it has
 * one, and its schedule.
 */
export function PluginJobItem({ job, anchor }: { job: PluginJob; anchor: string | undefined }) {
  return (
    <li className="flex flex-col items-start">
      {anchor === undefined ? (
        job.name
      ) : (
        <Link to="/admin/jobs" hash={anchor} className={textLink}>
          {job.name}
        </Link>
      )}
      <span className="text-sm text-muted-foreground">{scheduleWords(job.schedule)}</span>
    </li>
  );
}
