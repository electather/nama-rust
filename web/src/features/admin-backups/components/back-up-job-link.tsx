import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import type { Job } from "@/gen/nama/v1/job_pb";
import { detailSearch } from "@/shared/lib/settings-address";
import { textLink } from "@/shared/lib/text-link-classes";

/** A link to the Back up `job` in Settings' Jobs section, where its schedule changes and its Runs' logs are. */
export function BackUpJobLink({ job, children }: { job: Job; children: ReactNode }) {
  return (
    <Link
      to="."
      search={(rest) => ({
        ...rest,
        ...detailSearch({ kind: "job", pluginId: job.pluginId, jobId: job.id }),
      })}
      className={textLink}
    >
      {children}
    </Link>
  );
}
