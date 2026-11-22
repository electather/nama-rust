import { TriangleAlert } from "lucide-react";

import { type Job, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** Why `job`'s last Run failed, on red, or nothing unless it did. */
export function LastRunFailed({ job: { lastRun } }: { job: Job }) {
  if (lastRun?.outcome !== JobRunOutcome.FAILED) {
    return null;
  }
  return (
    <p className="flex gap-2.5 rounded-lg bg-destructive/10 px-3 py-2.5 text-sm text-pretty">
      <TriangleAlert
        aria-hidden
        strokeWidth={1.5}
        className="mt-0.5 size-4 shrink-0 text-destructive"
      />
      <span>The last Run failed: {lastRun.error}</span>
    </p>
  );
}
