import { withKind } from "@/features/admin-jobs/lib/schedule-picks";
import { KindChoices } from "@/features/settings/components/schedule-picker/kind-choices";
import { PickFields } from "@/features/settings/components/schedule-picker/pick-fields";
import { SchedulePreview } from "@/features/settings/components/schedule-picker/schedule-preview";
import { useScheduleDraft } from "@/features/settings/hooks/use-schedule-draft";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * `job`'s schedule, picked as how often and then a time, days or a date, or typed as cron under
 * Custom, previewed in words and its next Runs, and saved as soon as it's picked. `current` is the
 * one it runs on, and `error` why the Server refused the last one sent.
 */
export function SchedulePicker({
  job,
  current,
  error,
  onSave,
}: {
  job: Job;
  current: string;
  error: string | undefined;
  onSave: (schedule: string) => void;
}) {
  const draft = useScheduleDraft({ job, current, refused: error, save: onSave });
  const { pick, change, problem } = draft;
  return (
    <div data-setting="schedule" className="flex flex-col gap-3 border-t py-3">
      <KindChoices
        value={pick.kind}
        onChange={(kind) => {
          change(withKind(pick, kind));
        }}
      />
      <PickFields
        pick={pick}
        invalid={problem !== undefined}
        onChange={(part) => {
          change({ ...pick, ...part });
        }}
      />
      <SchedulePreview
        job={job}
        cron={draft.cron}
        next={draft.next}
        problem={problem}
        onReset={draft.handleReset}
      />
    </div>
  );
}
