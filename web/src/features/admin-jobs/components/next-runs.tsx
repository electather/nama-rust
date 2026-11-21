import { At } from "@/features/admin-jobs/components/at";

/** When a Job's schedule next fires, soonest first. */
export function NextRuns({ times }: { times: bigint[] }) {
  if (times.length === 0) {
    return null;
  }
  return (
    <ol aria-label="Next runs" className="text-sm text-muted-foreground tabular-nums">
      {times.map((ms) => (
        <li key={ms.toString()}>
          Next: <At ms={ms} />
        </li>
      ))}
    </ol>
  );
}
