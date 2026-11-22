import { useState } from "react";

import { useSchedulePreview } from "@/features/admin-jobs/data/use-schedule-preview";
import {
  cronOf,
  pickOf,
  pickProblem,
  type SchedulePick,
} from "@/features/admin-jobs/lib/schedule-picks";
import type { Job } from "@/gen/nama/v1/job_pb";
import { serverMessage } from "@/shared/lib/forms";

/** The Server's message for `error`, or nothing without one. */
const said = (error: unknown) => (error === null ? undefined : serverMessage(error));

/**
 * The schedule being picked for `job`, from `current`, the one it runs on, saved through `save` as
 * soon as a pick makes a whole one that differs: as the picker holds it, as cron, the next times it
 * fires and what's wrong with it, if anything, which `refused`, why the Server refused the last one
 * sent, stands in for.
 */
export function useScheduleDraft({
  job,
  current,
  refused,
  save,
}: {
  job: Job;
  current: string;
  refused: string | undefined;
  save: (schedule: string) => void;
}) {
  const [pick, setPick] = useState(() => pickOf(current));
  const cron = cronOf(pick);
  const missing = pickProblem(pick);
  const preview = useSchedulePreview(cron, missing !== undefined);
  const change = (next: SchedulePick) => {
    setPick(next);
    const wanted = cronOf(next);
    if (pickProblem(next) === undefined && wanted !== current) {
      save(wanted);
    }
  };
  return {
    pick,
    cron,
    change,
    next: preview.next,
    problem: missing ?? said(preview.error) ?? refused,
    handleReset: () => {
      change(pickOf(job.defaultSchedule));
    },
  };
}
