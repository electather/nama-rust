import { RotateCcw } from "lucide-react";
import { Button } from "nama-ui/components/button";

import type { Job } from "@/gen/nama/v1/job_pb";
import { scheduleWords } from "@/shared/lib/schedule-words";

/** Whether `cron` is the schedule `job`'s Plugin gives it, and if it isn't, what that is, with a way back to it. */
export function PluginSchedule({
  job,
  cron,
  onReset,
}: {
  job: Job;
  cron: string;
  onReset: () => void;
}) {
  if (cron === job.defaultSchedule) {
    return (
      <p className="text-xs text-muted-foreground">
        This is the schedule {job.pluginName} gives it.
      </p>
    );
  }
  return (
    <p className="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
      {job.pluginName}’s own: {scheduleWords(job.defaultSchedule)}.
      <Button variant="ghost" onClick={onReset}>
        <RotateCcw aria-hidden strokeWidth={1.5} />
        Use it
      </Button>
    </p>
  );
}
