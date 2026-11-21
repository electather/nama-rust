import { ChevronRight } from "lucide-react";

import { At } from "@/features/admin-jobs/components/at";
import { RunOutcome } from "@/features/admin-jobs/components/run-outcome";
import type { JobRun } from "@/gen/nama/v1/job_pb";

/** What a closed Run shows: when it started and how it ended. */
export function RunSummary({ run }: { run: JobRun }) {
  return (
    <summary className="flex cursor-pointer list-none items-center gap-3 rounded-sm text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50 [&::-webkit-details-marker]:hidden">
      <span className="flex-1 tabular-nums">
        <At ms={run.startedAtMs} />
      </span>
      <RunOutcome outcome={run.outcome} />
      <ChevronRight
        aria-hidden
        className="size-4 transition-transform duration-150 ease-out-expo group-open/run:rotate-90 motion-reduce:transition-none"
      />
    </summary>
  );
}
