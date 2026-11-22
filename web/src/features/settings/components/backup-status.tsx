import { create } from "@bufbuild/protobuf";

import { BackingUp } from "@/features/settings/components/backing-up";
import { FailedBackUp } from "@/features/settings/components/failed-back-up";
import { LastBackUp } from "@/features/settings/components/last-back-up";
import { UpkeepOff } from "@/features/settings/components/upkeep-off";
import { type Job, JobRunOutcome, JobRunSchema } from "@/gen/nama/v1/job_pb";
import type { Backup } from "@/gen/nama/v1/server_pb";

// A Job that has never run, as one with no outcome.
const NEVER_RUN = create(JobRunSchema);

/**
 * How the Server's Backups stand, at the top of their pane: the `newest` and when the Back up `job`
 * next writes one, that it's writing one or that its last Run failed, or with no `job`, that
 * Upkeep is disabled.
 */
export function BackupStatus({
  job,
  newest,
}: {
  job: Job | undefined;
  newest: Backup | undefined;
}) {
  if (job === undefined) {
    return <UpkeepOff />;
  }
  const { lastRun: run = NEVER_RUN } = job;
  if (run.outcome === JobRunOutcome.FAILED) {
    return <FailedBackUp job={job} run={run} newest={newest} />;
  }
  if (run.outcome === JobRunOutcome.RUNNING) {
    return <BackingUp job={job} run={run} />;
  }
  return <LastBackUp job={job} newest={newest} />;
}
