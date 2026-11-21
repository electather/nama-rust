import { useId } from "react";

import type { JobRun } from "@/gen/nama/v1/job_pb";
import { LogLines } from "@/shared/components/log-lines";

/** The Plugin Log lines a Run's Plugin wrote during it, oldest first, or that none are kept. */
export function RunLog({ run }: { run: JobRun }) {
  const id = useId();
  return (
    <section aria-labelledby={id} className="mt-4">
      <h4 id={id} className="text-sm font-medium">
        Plugin Log
      </h4>
      <LogLines
        lines={run.logLines}
        labelledBy={id}
        empty="No lines kept. A Plugin’s last 100 are kept until it’s disabled or the Server restarts."
      />
    </section>
  );
}
