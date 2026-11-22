import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Job } from "@/gen/nama/v1/job_pb";
import { jobSearch } from "@/shared/lib/job-key";
import { textLink } from "@/shared/lib/text-link-classes";

/** A link to the Back up `job` in Settings' Jobs section, where its schedule changes and its Runs' logs are. */
export function BackUpJobLink({ job, children }: { job: Job; children: ReactNode }) {
  return (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...jobSearch(job.pluginId, job.id) })}
      className={textLink}
    >
      {children}
    </Link>
  );
}
