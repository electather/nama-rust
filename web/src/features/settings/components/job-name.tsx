import { Link } from "@tanstack/react-router";
import { cn } from "cn";
import { CalendarClock } from "lucide-react";

import type { Job } from "@/gen/nama/v1/job_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { detailSearch } from "@/shared/lib/settings-address";

/** A schedule's words without the Server's clock, which the table says once under its rows. */
const withoutZone = (words: string) => words.replace(/,? Server time$/u, "");

/**
 * A Job's name on its row, a link stretched over the row, over its schedule, dimmed while it's
 * off, and why Run now didn't start it, if it didn't.
 */
export function JobName({ job, failed }: { job: Job; failed: string | undefined }) {
  const name = (
    <Link
      to="."
      search={(rest) => ({
        ...rest,
        ...detailSearch({ kind: "job", pluginId: job.pluginId, jobId: job.id }),
      })}
      className="truncate outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {job.name}
    </Link>
  );
  return (
    <span className={cn("flex items-center gap-3", !job.enabled && "opacity-60")}>
      <IconTile Icon={CalendarClock} />
      <span className="flex min-w-0 flex-col">
        {name}
        <span className="text-xs text-muted-foreground">
          {withoutZone(scheduleWords(job.schedule))}
        </span>
        <span role="alert" className="text-xs text-destructive empty:hidden">
          {failed}
        </span>
      </span>
    </span>
  );
}
