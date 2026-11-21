import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** Why the Back up Job's `lastRun` failed, if it did, so the Admin sees it without opening it. */
export function LastBackUpFailed({ lastRun }: { lastRun: JobRun | undefined }) {
  if (lastRun?.outcome !== JobRunOutcome.FAILED) {
    return null;
  }
  return (
    <p className="max-w-prose text-sm text-pretty text-destructive">
      The last Backup failed: {lastRun.error}
    </p>
  );
}
