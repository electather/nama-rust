import { useId } from "react";

import { JobHeader } from "@/features/admin-jobs/components/job-header";
import { JobRunsLoader } from "@/features/admin-jobs/components/job-runs-loader";
import { ScheduleRow } from "@/features/admin-jobs/components/schedule-row";
import { SaveToast } from "@/features/settings/components/save-toast";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { SwitchRow } from "@/features/settings/components/switch-row";
import { useJobConfig } from "@/features/settings/hooks/use-job-config";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * The Job `job` in place of the list: what it does and when it runs, whether it's on and its own
 * schedule, saved as they're changed, and its Runs, newest first.
 */
export function JobDetail({ job }: { job: Job }) {
  const titleId = useId();
  const { values, words, edit, dismiss } = useJobConfig(job);
  const when = (
    <SettingGroup title="When it runs">
      <SwitchRow
        label="Enabled"
        checked={values.enabled}
        onChange={(enabled) => {
          edit({ enabled });
        }}
      />
      <ScheduleRow
        job={job}
        saved={values.schedule}
        error={words.schedule}
        onSave={(schedule) => {
          edit({ schedule });
        }}
      />
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
        className="flex flex-col gap-6 p-4 md:px-6 md:pt-2 md:pb-6"
      >
        <JobHeader job={job} titleId={titleId} />
        {when}
        {runs}
      </section>
      <SaveToast says={words.form} onDismiss={dismiss} />
    </>
  );
}
