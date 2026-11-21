import { At } from "@/features/admin-jobs/components/at";
import { Fact } from "@/features/admin-jobs/components/fact";
import { duration, endUnknown } from "@/features/admin-jobs/lib/run-times";
import { startedBy } from "@/features/admin-jobs/lib/started-by";
import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** A Run's detail: what started it, when it started and ended, how long it took and its error. */
export function RunFacts({ run }: { run: JobRun }) {
  return (
    <dl className="mt-3 flex flex-col gap-1 text-sm">
      <Fact term="Started by">{startedBy(run)}</Fact>
      <Fact term="Started">
        <At ms={run.startedAtMs} />
      </Fact>
      <Fact term="Ended">
        {run.endedAtMs === undefined ? endUnknown(run.outcome) : <At ms={run.endedAtMs} />}
      </Fact>
      {run.endedAtMs !== undefined && (
        <Fact term="Took">{duration(run.startedAtMs, run.endedAtMs)}</Fact>
      )}
      {run.outcome === JobRunOutcome.FAILED && <Fact term="Error">{run.error}</Fact>}
    </dl>
  );
}
