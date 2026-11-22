import type { Job } from "@/gen/nama/v1/job_pb";
import { TypedSettingRow } from "@/shared/components/typed-setting-row";
import { scheduleWords } from "@/shared/lib/schedule-words";

/**
 * `job`'s own cron schedule on its row, `saved` empty for its Plugin's, saved once it's left. A
 * Job its Plugin gives no schedule runs only when an Admin runs it, so it can't be given one.
 */
export function ScheduleRow({
  job,
  saved,
  error,
  onSave,
}: {
  job: Job;
  saved: string;
  /** Why the Server refused the schedule last sent, if it did. */
  error: string | undefined;
  onSave: (schedule: string) => void;
}) {
  if (job.defaultSchedule === "") {
    return (
      <p className="py-3.5 text-sm text-muted-foreground">It runs only when an Admin runs it.</p>
    );
  }
  return (
    <TypedSettingRow
      setting="schedule"
      label="Schedule"
      hint={`Minute, hour, day of month, month and day of week, in Server time. Empty keeps its Plugin’s: ${scheduleWords(job.defaultSchedule)}.`}
      saved={saved}
      error={error}
      input={{ placeholder: job.defaultSchedule, autoComplete: "off", spellCheck: false }}
      onSave={onSave}
    />
  );
}
