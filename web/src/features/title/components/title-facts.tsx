import { metadataLine } from "@/shared/lib/title-facts";

/**
 * A title's facts in one line, such as "Movie · Drama · 2019", skipping unknowns, then each of
 * `boxed` in a box of its own, such as its Content Rating and "4K".
 */
export function TitleFacts({
  facts,
  boxed,
}: {
  facts: (string | undefined)[];
  boxed: (string | undefined)[];
}) {
  return (
    <p className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
      <span>{metadataLine(...facts)}</span>
      {boxed.filter(Boolean).map((fact) => (
        <span key={fact} className="rounded-sm border border-input px-1.5 text-xs/5 font-medium">
          {fact}
        </span>
      ))}
    </p>
  );
}
