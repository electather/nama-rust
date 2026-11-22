import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Job } from "@/gen/nama/v1/job_pb";
import { jobAnchor } from "@/shared/lib/job-anchor";
import { textLink } from "@/shared/lib/text-link-classes";

/** A link to the Back up `job` on the Jobs page, where its schedule changes and its Runs' logs are. */
export function BackUpJobLink({ job, children }: { job: Job; children: ReactNode }) {
  return (
    <Link to="/admin/jobs" hash={jobAnchor(job.pluginId, job.id)} className={textLink}>
      {children}
    </Link>
  );
}
