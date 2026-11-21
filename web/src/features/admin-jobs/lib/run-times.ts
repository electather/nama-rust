import { type Job, type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** `ms`, milliseconds since the Unix epoch, as a date and time in the browser's time zone. */
export function formatAt(ms: bigint) {
  return new Date(Number(ms)).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

/** When `job` next runs and when it last ran, in a line. */
export function nextAndLast({ nextRunsAtMs, lastRun }: Job) {
  const [next] = nextRunsAtMs;
  const last = lastRun ? `Last run ${formatAt(lastRun.startedAtMs)}` : "Never run";
  return next === undefined ? last : `Next run ${formatAt(next)} · ${last}`;
}

/** When `run` ended, or why that isn't known. */
export function ended({ outcome, endedAtMs }: JobRun) {
  if (endedAtMs !== undefined) {
    return formatAt(endedAtMs);
  }
  return outcome === JobRunOutcome.RUNNING
    ? "Still running"
    : "Not known: the Server stopped during it";
}

const seconds = new Intl.NumberFormat(undefined, {
  style: "unit",
  unit: "second",
  unitDisplay: "long",
});

const minutes = new Intl.NumberFormat(undefined, {
  style: "unit",
  unit: "minute",
  unitDisplay: "long",
});

/** How long a Run that started at `startedAtMs` and ended at `endedAtMs` took. */
export function duration(startedAtMs: bigint, endedAtMs: bigint) {
  const total = Math.round(Number(endedAtMs - startedAtMs) / 1000);
  if (total < 1) {
    return "Under a second";
  }
  const [whole, rest] = [Math.floor(total / 60), total % 60];
  const inSeconds = seconds.format(rest);
  return whole === 0 ? inSeconds : `${minutes.format(whole)} ${inSeconds}`;
}
