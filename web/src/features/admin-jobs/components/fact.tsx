import type { ReactNode } from "react";

/** One term of a Run's detail and its value. */
export function Fact({ term, children }: { term: string; children: ReactNode }) {
  return (
    <div className="flex gap-4">
      <dt className="w-24 shrink-0 text-muted-foreground">{term}</dt>
      <dd className="min-w-0 wrap-break-word whitespace-pre-wrap tabular-nums">{children}</dd>
    </div>
  );
}
