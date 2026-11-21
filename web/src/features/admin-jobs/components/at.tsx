import { formatAt } from "@/features/admin-jobs/lib/run-times";

/** A time, `ms` since the Unix epoch, shown in the browser's time zone. */
export function At({ ms }: { ms: bigint }) {
  return <time dateTime={new Date(Number(ms)).toISOString()}>{formatAt(ms)}</time>;
}
