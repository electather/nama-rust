import { useId } from "react";

import { RowTitleItem } from "@/features/home/components/row-title-item";
import { useHomeRowTitles } from "@/features/home/data/use-home-row-titles";
import { rowTitleKey, rowTitles } from "@/features/home/lib/row-titles";
import type { HomeRow } from "@/gen/nama/v1/browse_pb";
import { PosterGrid } from "@/shared/components/poster/poster-grid";

/** One Plugin's row under its heading, or nothing until its titles load or if it has none. */
export function HomeRowSection({ row }: { row: HomeRow }) {
  const headingId = useId();
  const titles = rowTitles(useHomeRowTitles(row) ?? []);
  if (titles.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby={headingId} className="mt-10">
      <h2 id={headingId} className="font-medium">
        {row.heading}
      </h2>
      <PosterGrid empty={null}>
        {titles.map((title) => (
          <RowTitleItem key={rowTitleKey(title.value)} title={title} />
        ))}
      </PosterGrid>
    </section>
  );
}
