import { metadataLine } from "@/shared/lib/title-facts";

/**
 * A title's facts in one Label line, such as "2022 · 2 Seasons", skipping unknowns, then each of
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
    <p className="flex flex-wrap items-center gap-2 text-xs text-foreground/80">
      <span>{metadataLine(...facts)}</span>
      {boxed.filter(Boolean).map((fact) => (
        <span key={fact} className="rounded-sm border border-input px-1.5 leading-4.5 font-medium">
          {fact}
        </span>
      ))}
    </p>
  );
}
