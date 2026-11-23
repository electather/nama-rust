import { JobFact } from "@/features/admin-jobs/components/job-fact";
import { LastRun } from "@/features/admin-jobs/components/last-run";
import { NextRun } from "@/features/admin-jobs/components/next-run";
import { UsualTimeLoader } from "@/features/admin-jobs/components/usual-time-loader";
import type { Job } from "@/gen/nama/v1/job_pb";

/** Three facts in a row, one under another on a phone: when `job` runs next, how it last went and how long it usually takes. */
export function JobFacts({ job }: { job: Job }) {
  return (
    <dl className="mt-1 grid gap-px overflow-hidden rounded-lg border bg-border text-sm md:grid-cols-3">
      <JobFact term="Next run">
        <NextRun job={job} />
      </JobFact>
      <JobFact term="Last run">
        <LastRun job={job} />
      </JobFact>
      <JobFact term="Usually takes">
        <UsualTimeLoader job={job} />
      </JobFact>
    </dl>
  );
}
