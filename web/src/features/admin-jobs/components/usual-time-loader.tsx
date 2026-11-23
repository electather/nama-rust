import { Suspense } from "react";

import { UsualTime } from "@/features/admin-jobs/components/usual-time";
import type { Job } from "@/gen/nama/v1/job_pb";
import { ErrorBoundary } from "@/shared/components/error-boundary";

/** How long `job` usually takes, "…" while its Runs load, or "Not known" if they can't be read. */
export function UsualTimeLoader({ job }: { job: Job }) {
  return (
    <ErrorBoundary failed="Not known">
      <Suspense fallback="…">
        <UsualTime job={job} />
      </Suspense>
    </ErrorBoundary>
  );
}
