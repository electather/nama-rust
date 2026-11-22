import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** How long `run` took, in ms. */
export const tookMs = (run: JobRun) => Number((run.endedAtMs ?? run.startedAtMs) - run.startedAtMs);

/** The last 20 of `runs`, newest first, that ended, oldest first. */
export function endedOldestFirst(runs: JobRun[]) {
  return (
    runs
      .filter((run) => run.endedAtMs !== undefined)
      .slice(0, 20)
      // oxlint-disable-next-line unicorn/no-array-reverse -- reverses the array just built, and the lib target has no toReversed
      .reverse()
  );
}

// Red only for Failed, as DESIGN.md keeps it the only status colour; quiet for the rest that didn't succeed.
const FILLS: Partial<Record<JobRunOutcome, string>> = {
  [JobRunOutcome.FAILED]: "fill-destructive",
  [JobRunOutcome.SUCCEEDED]: "fill-foreground/50",
};

/** A Run's bar's fill. */
export const fillOf = (outcome: JobRunOutcome) => FILLS[outcome] ?? "fill-muted-foreground/60";

/** How many of `runs` failed. */
export const failedOf = (runs: JobRun[]) =>
  runs.filter((run) => run.outcome === JobRunOutcome.FAILED).length;
