import { outcomes } from "@/features/admin-jobs/lib/outcomes";
import type { JobRunOutcome } from "@/gen/nama/v1/job_pb";
import { StateLabel } from "@/shared/components/state-label";

/** How a Job Run ended, or that it's running, as a labelled pill: none for an outcome it doesn't know. */
export function RunOutcome({ outcome }: { outcome: JobRunOutcome }) {
  const shown = outcomes[outcome];
  return shown && <StateLabel {...shown} />;
}
