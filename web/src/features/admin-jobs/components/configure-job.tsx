import { Button } from "nama-ui/components/button";
import { useRef, useState } from "react";

import { JobSettings } from "@/features/admin-jobs/components/job-settings";
import { useConfigureJob } from "@/features/admin-jobs/data/use-configure-job";
import { jobRefusal } from "@/features/admin-jobs/lib/job-refusal";
import type { Job } from "@/gen/nama/v1/job_pb";
import { ConfirmModal } from "@/shared/components/confirm-dialog/confirm-modal";
import { formText } from "@/shared/lib/forms";

/** A Configure button that opens a dialog turning `job` on or off and setting its schedule. */
export function ConfigureJob({ job }: { job: Job }) {
  const configure = useConfigureJob();
  const dialog = useRef<HTMLDialogElement>(null);
  const [openings, setOpenings] = useState(0);
  return (
    <>
      <Button
        variant="ghost"
        aria-label={`Configure ${job.name}`}
        aria-haspopup="dialog"
        onClick={() => {
          setOpenings(openings + 1);
          dialog.current?.showModal();
        }}
      >
        Configure
      </Button>
      <ConfirmModal
        ref={dialog}
        openings={openings}
        action="Save"
        title={`Configure ${job.name}`}
        description="Turn it off, or run it on a schedule of your own in place of its Plugin’s."
        onConfirm={async (typed) => {
          await configure.mutateAsync({
            pluginId: job.pluginId,
            jobId: job.id,
            enabled: typed.get("enabled") === "on",
            schedule: formText(typed, "schedule"),
          });
        }}
        refusal={(error) => jobRefusal(error, "save the Job")}
      >
        {/* Only once opened, so its preview asks the Server only when someone looks. */}
        {openings > 0 && <JobSettings job={job} />}
      </ConfirmModal>
    </>
  );
}
