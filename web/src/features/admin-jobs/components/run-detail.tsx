import { RunFacts } from "@/features/admin-jobs/components/run-facts";
import { RunSummary } from "@/features/admin-jobs/components/run-summary";
import type { JobRun } from "@/gen/nama/v1/job_pb";

/** One Run in a Job's history: its outcome and start, opening to the rest. */
export function RunDetail({ run }: { run: JobRun }) {
  return (
    <li className="border-b py-2 last:border-b-0">
      <details className="group/run">
        <RunSummary run={run} />
        <RunFacts run={run} />
      </details>
    </li>
  );
}
