import { useEffect, useRef } from "react";

import { NextRuns } from "@/features/admin-jobs/components/next-runs";
import type { Job } from "@/gen/nama/v1/job_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";

/** The top of a Job's view: its name, what it does, its schedule and when it next runs. */
export function JobHeader({ job, titleId }: { job: Job; titleId: string }) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  return (
    <header className="flex flex-col gap-2">
      <h3
        ref={title}
        id={titleId}
        tabIndex={-1}
        className="text-lg font-medium text-balance outline-none"
      >
        {job.name}
      </h3>
      {job.description !== "" && (
        <p className="max-w-prose text-sm/6 text-pretty text-muted-foreground">{job.description}</p>
      )}
      <p className="text-sm">{scheduleWords(job.schedule)}</p>
      <NextRuns times={job.nextRunsAtMs} />
    </header>
  );
}
