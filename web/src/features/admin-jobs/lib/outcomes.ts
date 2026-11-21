import { CircleCheck, CircleMinus, CircleX, LoaderCircle, Timer } from "lucide-react";

import { JobRunOutcome } from "@/gen/nama/v1/job_pb";
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
