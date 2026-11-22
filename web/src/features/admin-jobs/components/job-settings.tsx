import { Button } from "nama-ui/components/button";
import { useState } from "react";

import { EnabledSwitch } from "@/features/admin-jobs/components/enabled-switch";
import { ScheduleField } from "@/features/admin-jobs/components/schedule-field";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * What Configure asks of `job`: whether it's on and, if its Plugin gives it a schedule, one of its
 * own. Reset to default puts both back as its Plugin has them, for Save to keep.
 */
export function JobSettings({ job }: { job: Job }) {
  const [enabled, setEnabled] = useState(job.enabled);
  const [schedule, setSchedule] = useState(
    job.schedule === job.defaultSchedule ? "" : job.schedule,
  );
  return (
    <>
      <EnabledSwitch on={enabled} onChange={setEnabled} />
      {job.defaultSchedule === "" ? (
        <p className="text-sm text-muted-foreground">It runs only when an Admin runs it.</p>
      ) : (
        <ScheduleField
          value={schedule}
          pluginSchedule={job.defaultSchedule}
          onChange={setSchedule}
        />
      )}
      <div className="self-start">
        <Button
          variant="link"
          size="link"
          onClick={() => {
            setEnabled(true);
            setSchedule("");
          }}
        >
          Reset to default
        </Button>
      </div>
    </>
  );
}
