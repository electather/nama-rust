import { Link } from "@tanstack/react-router";
import { CalendarClock } from "lucide-react";

import { JobState } from "@/features/admin-jobs/components/job-state";
import { RunNowButton } from "@/features/admin-jobs/components/run-now-button";
import { useRunJob } from "@/features/admin-jobs/data/use-run-job";
import { runRefused } from "@/features/admin-jobs/lib/run-refused";
import { nextAndLast } from "@/features/admin-jobs/lib/run-times";
import { ItemRow } from "@/features/settings/components/item-row";
import type { Job } from "@/gen/nama/v1/job_pb";
import { IconTile } from "@/shared/components/icon-tile";
import { jobKey } from "@/shared/lib/job-key";
import { scheduleWords } from "@/shared/lib/schedule-words";

/**
 * One Job on the list: its name over its schedule and its next and last Runs, and at the end
 * whether it's off, how its last Run ended and Run now. All of the row but those opens it.
 */
export function JobItem({ job }: { job: Job }) {
  const run = useRunJob();
  const name = (
    <Link
      to="."
      search={(rest) => ({ ...rest, job: jobKey(job.pluginId, job.id) })}
      className="outline-none after:absolute after:inset-0 after:rounded-lg focus-visible:after:ring-3 focus-visible:after:ring-ring/50"
    >
      {job.name}
    </Link>
  );
  return (
    <ItemRow
      beside
      linked
      start={<IconTile Icon={CalendarClock} />}
      name={name}
      meta={`${scheduleWords(job.schedule)} · ${nextAndLast(job)}`}
      failed={run.isError ? runRefused(run.error) : undefined}
    >
      <JobState job={job} />
      <RunNowButton job={job} run={run} />
    </ItemRow>
  );
}
