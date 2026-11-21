import { Fact } from "@/features/admin-jobs/components/fact";
import { duration, ended, formatAt } from "@/features/admin-jobs/lib/run-times";
import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** A Run's detail: what started it, when it started and ended, how long it took and its error. */
export function RunFacts({ run }: { run: JobRun }) {
  return (
    <dl className="mt-3 flex flex-col gap-1 text-sm">
      {/* Every Run is the schedule's until an Admin can start one. */}
      <Fact term="Started by">Schedule</Fact>
      <Fact term="Started">{formatAt(run.startedAtMs)}</Fact>
      <Fact term="Ended">{ended(run)}</Fact>
      {run.endedAtMs !== undefined && (
        <Fact term="Took">{duration(run.startedAtMs, run.endedAtMs)}</Fact>
      )}
      {run.outcome === JobRunOutcome.FAILED && <Fact term="Error">{run.error}</Fact>}
    </dl>
  );
}
