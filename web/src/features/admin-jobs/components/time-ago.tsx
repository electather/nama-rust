import { formatAt, relative } from "@/features/admin-jobs/lib/run-times";

/** A time, `ms` since the Unix epoch, from now, such as "3 hours ago", with the exact time on hover. */
export function TimeAgo({ ms }: { ms: bigint }) {
  return (
    <time dateTime={new Date(Number(ms)).toISOString()} title={formatAt(ms)}>
      {relative(ms)}
    </time>
  );
}
