import { useRunRefusal } from "@/features/admin-jobs/data/use-run-refusal";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Why the last Run now of `job`, from wherever it was pressed, didn't start, or nothing. */
export function RunRefusal({ job }: { job: Job }) {
  return (
    <p role="alert" className="text-sm text-destructive empty:hidden">
      {useRunRefusal(job)}
    </p>
  );
}
