import { CalendarClock } from "lucide-react";

import { NextRunTimes } from "@/features/settings/components/schedule-picker/next-run-times";
import { PluginSchedule } from "@/features/settings/components/schedule-picker/plugin-schedule";
import type { Job } from "@/gen/nama/v1/job_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";

/**
 * What the picked schedule does, `cron`: in words and the next times the Server says it fires, or
 * the `problem` with it; and whether it's the Plugin's own, with a way back to that.
 */
export function SchedulePreview({
  job,
  cron,
  next,
  problem,
  onReset,
}: {
  job: Job;
  cron: string;
  next: bigint[] | undefined;
  problem: string | undefined;
  onReset: () => void;
}) {
  const said =
    problem === undefined ? (
      <p className="flex items-center gap-2 font-medium">
        <CalendarClock aria-hidden strokeWidth={1.5} className="size-4 shrink-0" />
        {scheduleWords(cron)}
      </p>
    ) : (
      <p role="alert" className="text-destructive">
        {problem}
      </p>
    );
  return (
    <div
      aria-live="polite"
      className="flex flex-col gap-1.5 rounded-lg bg-card px-3 py-2.5 text-sm"
    >
      {said}
      <NextRunTimes next={problem === undefined ? next : undefined} />
      <PluginSchedule job={job} cron={cron} onReset={onReset} />
    </div>
  );
}
