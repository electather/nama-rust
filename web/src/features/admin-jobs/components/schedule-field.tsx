import { NextRuns } from "@/features/admin-jobs/components/next-runs";
import { useSchedulePreview } from "@/features/admin-jobs/data/use-schedule-preview";
import { FormField } from "@/shared/components/form-field/form-field";
import { scheduleWords } from "@/shared/lib/schedule-words";

/**
 * A Job's own cron schedule, empty for its Plugin's, with when it would next run or why the Server
 * can't read it.
 */
export function ScheduleField({
  value,
  pluginSchedule,
  onChange,
}: {
  value: string;
  pluginSchedule: string;
  onChange: (value: string) => void;
}) {
  const preview = useSchedulePreview(value === "" ? pluginSchedule : value);
  return (
    <div className="flex flex-col gap-2">
      <FormField
        label="Schedule"
        name="schedule"
        required={false}
        value={value}
        placeholder={pluginSchedule}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => {
          onChange(event.target.value);
        }}
        error={preview.error}
        hint={`Minute, hour, day of month, month and day of week, in Server time. Empty keeps its Plugin’s: ${scheduleWords(pluginSchedule)}.`}
      />
      <NextRuns times={preview.nextRunsAtMs} />
    </div>
  );
}
