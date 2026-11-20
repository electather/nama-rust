import type { ReactNode } from "react";

/** One of a title's Details: its name in Label, over its value. */
export function DetailFact({ term, value }: { term: string; value: ReactNode }) {
  return (
    <div>
      <dt className="text-sm text-muted-foreground">{term}</dt>
      <dd className="text-foreground/80">{value}</dd>
    </div>
  );
}
