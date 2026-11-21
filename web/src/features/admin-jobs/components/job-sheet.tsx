import { type RefObject, useId } from "react";

import { JobSheetBody } from "@/features/admin-jobs/components/job-sheet-body";
import type { Job } from "@/gen/nama/v1/job_pb";

/**
 * A Job's sheet, opened with `showModal()` on `ref`: its description, schedule and Runs, which it
 * reads afresh each time it opens.
 */
export function JobSheet({
  ref,
  job,
  onClose,
}: {
  ref: RefObject<HTMLDialogElement | null>;
  job: Job | undefined;
  onClose: () => void;
}) {
  const id = useId();
  return (
    // ponytail: the native modal <dialog> traps focus, closes on Escape and returns focus itself.
    <dialog
      ref={ref}
      aria-labelledby={id}
      onClose={onClose}
      // DESIGN.md's Dialog, as a sheet down the end of the screen, the whole screen on a phone.
      className="my-0 ms-auto me-0 h-dvh max-h-dvh w-full max-w-full overscroll-contain border-s bg-background text-foreground shadow-lg backdrop:overflow-hidden backdrop:overscroll-contain backdrop:bg-scrim sm:max-w-md"
    >
      {job && <JobSheetBody job={job} headingId={id} close={() => ref.current?.close()} />}
    </dialog>
  );
}
