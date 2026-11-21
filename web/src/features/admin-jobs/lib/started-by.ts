import type { JobRun } from "@/gen/nama/v1/job_pb";

/** What started `run`: the Admin's name, or "Schedule". */
export function startedBy(run: JobRun) {
  return run.startedBy === "" ? "Schedule" : run.startedBy;
}
