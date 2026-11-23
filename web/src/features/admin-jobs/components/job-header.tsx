import { useEffect, useRef } from "react";

import { JobFacts } from "@/features/admin-jobs/components/job-facts";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * The top of a Job's view: what it does, and when it runs next, how it last went and how long it
 * takes. Its name is only for screen readers, as the crumbs over the pane already show it.
 */
export function JobHeader({ job, titleId }: { job: Job; titleId: string }) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  return (
    <header className="flex flex-col gap-1.5">
      <h3 ref={title} id={titleId} tabIndex={-1} className="sr-only">
        {job.name}
      </h3>
      {job.description !== "" && (
        <p className="max-w-prose text-sm/6 text-pretty text-muted-foreground">{job.description}</p>
      )}
      <JobFacts job={job} />
    </header>
  );
}
