import { dayAndTime } from "@/features/admin-jobs/lib/run-times";

/** The next times a schedule fires, or that it never does; nothing until the Server says. */
export function NextRunTimes({ next }: { next: bigint[] | undefined }) {
  if (next === undefined) {
    return null;
  }
  if (next.length === 0) {
    return <p>This schedule never comes round, so it would never run.</p>;
  }
  return (
    <ol aria-label="Next runs" className="flex flex-wrap gap-1.5 tabular-nums">
      {next.map((ms) => (
        <li key={ms.toString()} className="rounded-full bg-secondary px-2.5 py-0.5">
          {dayAndTime(ms)}
        </li>
      ))}
    </ol>
  );
}
