import { Link } from "@tanstack/react-router";
import { TriangleAlert } from "lucide-react";

import { TimeAgo } from "@/features/admin-jobs/components/time-ago";
import { type Job, JobRunOutcome } from "@/gen/nama/v1/job_pb";
import { jobKey } from "@/shared/lib/job-key";

/** Over the list, each Job whose last Run failed, linking to it, or nothing while none did. */
export function FailedJobs({ jobs }: { jobs: Job[] }) {
  const failed = jobs.filter((job) => job.lastRun?.outcome === JobRunOutcome.FAILED);
  if (failed.length === 0) {
    return null;
  }
  const items = failed.map((job) => (
    <li key={`${job.pluginId}:${job.id}`}>
      <span className="font-medium">{job.name}</span> failed{" "}
      <TimeAgo ms={job.lastRun?.startedAtMs ?? 0n} />.{" "}
      <Link
        to="."
        search={(rest) => ({ ...rest, job: jobKey(job.pluginId, job.id) })}
        className="underline underline-offset-3"
      >
        See what happened
      </Link>
    </li>
  ));
  return (
    <div className="flex gap-2.5 rounded-lg bg-destructive/10 px-3 py-2.5 text-sm">
      <TriangleAlert
        aria-hidden
        strokeWidth={1.5}
        className="mt-0.5 size-4 shrink-0 text-destructive"
      />
      <ul aria-label="Failed Jobs">{items}</ul>
    </div>
  );
}
