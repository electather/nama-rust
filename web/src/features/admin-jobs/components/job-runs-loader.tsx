import { Suspense } from "react";

import { JobRuns } from "@/features/admin-jobs/components/job-runs";
import type { Job } from "@/gen/nama/v1/job_pb";
import { ErrorBoundary } from "@/shared/components/error-boundary";

const failed = (
  <p className="text-muted-foreground">Couldn’t load its Runs. Check that the Server is running.</p>
);

/** A Job's Runs, a line while they load, or that they failed, which leaves the rest of the page. */
export function JobRunsLoader({ job }: { job: Job }) {
  return (
    <ErrorBoundary failed={failed}>
      <Suspense fallback={<p className="text-muted-foreground">Loading its Runs…</p>}>
        <JobRuns job={job} />
      </Suspense>
    </ErrorBoundary>
  );
}
