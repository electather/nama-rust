import { PosterShelf } from "@/features/home/components/poster-shelf";
import { RowFrame } from "@/features/home/components/row-frame";
import { useHomeRowTitles } from "@/features/home/data/use-home-row-titles";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";
import { rowTitles } from "@/shared/lib/row-titles";

/** One Plugin's row of posters under its heading, or nothing if it has no titles. */
export function RowShelf({ row, headingId }: { row: PluginHomeRow; headingId: string }) {
  const { data, hasNextPage, fetchNextPage, totalSize } = useHomeRowTitles(row);
  const titles = rowTitles(data.pages.flatMap((page) => page.titles));
  if (titles.length === 0 && !hasNextPage) {
    return null;
  }
  return (
    <RowFrame heading={row.heading} headingId={headingId}>
      <PosterShelf
        titles={titles}
        setSize={totalSize}
        next={{ has: hasNextPage, load: fetchNextPage }}
      />
    </RowFrame>
  );
}
