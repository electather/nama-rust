import type { ReactNode } from "react";

/** One of a Job's facts at the top of its view: a Label term over its value. */
export function JobFact({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5 bg-popover px-3 py-2">
      <dt className="text-xs text-muted-foreground">{term}</dt>
      <dd>{children}</dd>
    </div>
  );
}
