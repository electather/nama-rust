import { Link } from "@tanstack/react-router";

import type { Job } from "@/gen/nama/v1/job_pb";
import { jobSearch } from "@/shared/lib/job-key";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { textLink } from "@/shared/lib/text-link-classes";

/** When the Back up Job runs by itself, linking to it in Settings' Jobs section. */
export function BackUpSchedule({ job }: { job: Job }) {
  return (
    <p className="max-w-prose text-sm text-pretty text-muted-foreground">
      {job.enabled ? scheduleWords(job.schedule) : "Turned off: only when you back up now"}. Change
      when, or see each Backup’s log, on the{" "}
      <Link
        to="."
        search={(rest) => ({ ...rest, ...jobSearch(job.pluginId, job.id) })}
        className={textLink}
      >
        Back up Job
      </Link>
      .
    </p>
  );
}
