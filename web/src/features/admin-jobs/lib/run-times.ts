import { type JobRun, JobRunOutcome } from "@/gen/nama/v1/job_pb";

/** `ms`, milliseconds since the Unix epoch, as a date and time in the browser's time zone. */
export function formatAt(ms: bigint) {
  return new Date(Number(ms)).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

const RELATIVE = new Intl.RelativeTimeFormat(undefined, { numeric: "auto", style: "short" });

/** `ms` from `now`, both since the Unix epoch, in words, such as "in 3 hr." or "2 days ago". */
export function relative(ms: bigint, now = Date.now()) {
  const minutes = Math.round((Number(ms) - now) / 60_000);
  if (Math.abs(minutes) < 60) {
    return RELATIVE.format(minutes, "minute");
  }
  const hours = Math.round(minutes / 60);
  return Math.abs(hours) < 48
    ? RELATIVE.format(hours, "hour")
    : RELATIVE.format(Math.round(hours / 24), "day");
}

const DAY_MS = 86_400_000;

/** The start of `date`'s day, in ms since the Unix epoch. */
const midnight = (date: Date) =>
  new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
const NEAR = ["Yesterday", "Today", "Tomorrow"];

/** `ms` as its day, "Today" or a weekday within the week, and its time, such as "Tomorrow, 04:10". */
export function dayAndTime(ms: bigint, now = new Date()) {
  const at = new Date(Number(ms));
  const days = Math.round((midnight(at) - midnight(now)) / DAY_MS);
  const day =
    NEAR[days + 1] ??
    at.toLocaleDateString(
      undefined,
      days > 0 && days < 7 ? { weekday: "long" } : { day: "numeric", month: "short" },
    );
  return `${day}, ${at.toLocaleTimeString(undefined, { timeStyle: "short" })}`;
}

/** The middle of how long `runs` that succeeded took, in ms, or nothing before one has. */
export function usualMs(runs: JobRun[]) {
  const took = runs
    .filter((run) => run.outcome === JobRunOutcome.SUCCEEDED && run.endedAtMs !== undefined)
    .map((run) => Number((run.endedAtMs ?? 0n) - run.startedAtMs))
    // oxlint-disable-next-line unicorn/no-array-sort -- sorts the array just built, and the lib target has no toSorted
    .sort((a, b) => a - b);
  return took[Math.floor(took.length / 2)];
}

/** How long something usually takes, `ms`, roughly, such as "about 2 minutes". */
export function roughly(ms: number) {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 1) {
    return "Under a minute";
  }
  return minutes === 1 ? "About a minute" : `About ${inUnit("minute").format(minutes)}`;
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
