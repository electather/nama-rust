import { useId } from "react";

import { JobHeader } from "@/features/admin-jobs/components/job-header";
import { JobRunsLoader } from "@/features/admin-jobs/components/job-runs-loader";
import { LastRunFailed } from "@/features/admin-jobs/components/last-run-failed";
import { RunRefusal } from "@/features/admin-jobs/components/run-refusal";
import { SaveToast } from "@/features/settings/components/save-toast";
import { SchedulePicker } from "@/features/settings/components/schedule-picker/schedule-picker";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { useJobConfig } from "@/features/settings/hooks/use-job-config";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * The Job `job` in place of the list: what it does, when it next runs, how it last went and why
 * Run now, beside Settings' close, didn't start it, if it didn't; whether it's on, saved as it's switched, and its schedule, saved as it's picked; and its Runs.
 */
export function JobDetail({ job }: { job: Job }) {
  const titleId = useId();
  const { values, words, edit, dismiss } = useJobConfig(job);
  const scheduled = job.defaultSchedule !== "";
  const when = (
    <SettingGroup title="When it runs">
      <SwitchRow
        label="Enabled"
        hint={
          scheduled
            ? "When it’s off, it runs only when an Admin runs it."
            : "It runs only when an Admin runs it."
        }
        checked={values.enabled}
        onChange={(enabled) => {
          edit({ enabled });
        }}
      />
      {scheduled && (
        <SchedulePicker
          job={job}
          current={values.schedule === "" ? job.defaultSchedule : values.schedule}
          error={words.schedule}
          onSave={(schedule) => {
            // Its Plugin's own schedule is saved as none, so it follows the Plugin if that changes it.
            edit({ schedule: schedule === job.defaultSchedule ? "" : schedule });
          }}
        />
      )}
    </SettingGroup>
  );
  const runs = (
    <SettingGroup title="Runs">
      <JobRunsLoader job={job} />
    </SettingGroup>
  );
  return (
    <>
      <section
        aria-labelledby={titleId}
        className="flex flex-col gap-3 p-4 md:px-5 md:pt-1 md:pb-6"
      >
        <JobHeader job={job} titleId={titleId} />
        <RunRefusal job={job} />
        <LastRunFailed job={job} />
        {when}
        {runs}
      </section>
      <SaveToast says={words.form} onDismiss={dismiss} />
    </>
  );
}
