import { Link } from "@tanstack/react-router";

import type { PluginJob } from "@/gen/nama/v1/plugin_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { detailSearch } from "@/shared/lib/settings-address";
import { textLink } from "@/shared/lib/text-link-classes";

/**
 * One of a Plugin's Jobs: its name, linking to it in Settings' Jobs section while its Plugin,
 * `pluginId`, is enabled, and its schedule.
 */
export function PluginJobItem({ job, pluginId }: { job: PluginJob; pluginId: string | undefined }) {
  return (
    <li className="flex flex-col items-start">
      {pluginId === undefined ? (
        job.name
      ) : (
        <Link
          to="."
          search={(rest) => ({
            ...rest,
            ...detailSearch({ kind: "job", pluginId, jobId: job.id }),
          })}
          className={textLink}
        >
          {job.name}
        </Link>
      )}
      <span className="text-sm text-muted-foreground">{scheduleWords(job.schedule)}</span>
    </li>
  );
}
