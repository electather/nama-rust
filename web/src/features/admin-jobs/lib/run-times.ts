import { type Job, JobRunOutcome } from "@/gen/nama/v1/job_pb";

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

/** Why a Run with `outcome` has no end time. */
export function endUnknown(outcome: JobRunOutcome) {
  return outcome === JobRunOutcome.RUNNING
    ? "Still running"
    : "Not known: the Server stopped during it";
}

/** Formats a count of one unit in words, such as "2 minutes". */
const inUnit = (unit: "hour" | "minute" | "second") =>
  new Intl.NumberFormat(undefined, { style: "unit", unit, unitDisplay: "long" });

const UNITS = [
  { format: inUnit("hour"), seconds: 3600 },
  { format: inUnit("minute"), seconds: 60 },
  { format: inUnit("second"), seconds: 1 },
];

/**
 * How long a Run that started at `startedAtMs` and ended at `endedAtMs` took, in hours, minutes
 * and seconds, each left out when 0.
 */
export function duration(startedAtMs: bigint, endedAtMs: bigint) {
  let left = Math.round(Number(endedAtMs - startedAtMs) / 1000);
  if (left < 1) {
    return "Under a second";
  }
  const parts: string[] = [];
  for (const { format, seconds } of UNITS) {
    const count = Math.floor(left / seconds);
    left %= seconds;
    if (count > 0) {
      parts.push(format.format(count));
    }
  }
  return parts.join(" ");
}
