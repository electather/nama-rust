import { CircleCheck, CircleMinus, CircleX, LoaderCircle, Timer } from "lucide-react";

import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";
import type { Shown } from "@/shared/components/state-label";

/** How a Job Run's outcome is shown, and how much it stands out. */
export const outcomes: Partial<
  Record<JobRunOutcome, Shown & { tone: "quiet" | "warning" | "destructive" }>
> = {
  [JobRunOutcome.RUNNING]: { label: "Running", Icon: LoaderCircle, tone: "quiet" },
  [JobRunOutcome.SUCCEEDED]: { label: "Succeeded", Icon: CircleCheck, tone: "quiet" },
  [JobRunOutcome.FAILED]: { label: "Failed", Icon: CircleX, tone: "destructive" },
  [JobRunOutcome.TIMED_OUT]: { label: "Timed out", Icon: Timer, tone: "warning" },
  [JobRunOutcome.INTERRUPTED]: { label: "Interrupted", Icon: CircleMinus, tone: "warning" },
};

/** How often to look again while a Run is going, to show how it ends. */
export const RUNNING_POLL_MS = 2000;

/** Whether `run` is still going. */
export function isRunning(run: JobRun | undefined) {
  return run?.outcome === JobRunOutcome.RUNNING;
}
