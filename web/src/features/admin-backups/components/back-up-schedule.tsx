import { Link } from "@tanstack/react-router";

import type { Job } from "@/gen/nama/v1/job_pb";
import { jobAnchor } from "@/shared/lib/job-anchor";
import { scheduleWords } from "@/shared/lib/schedule-words";
import { textLink } from "@/shared/lib/text-link-classes";

/** When the Back up Job runs by itself, linking to it on the Jobs page. */
export function BackUpSchedule({ job }: { job: Job }) {
  return (
    <p className="max-w-prose text-sm text-pretty text-muted-foreground">
      {job.enabled ? scheduleWords(job.schedule) : "Turned off: only when you back up now"}. Change
      when, or see each Backup’s log, on the{" "}
      <Link to="/admin/jobs" hash={jobAnchor(job.pluginId, job.id)} className={textLink}>
        Back up Job
      </Link>
      .
    </p>
  );
}
