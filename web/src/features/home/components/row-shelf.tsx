import { RowFrame } from "@/features/home/components/row-frame";
import { RowPoster } from "@/features/home/components/row-poster";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { useHomeRowTitles } from "@/features/home/data/use-home-row-titles";
import { rowTile } from "@/features/home/lib/row-tiles";
import { rowTitleKey, rowTitles } from "@/features/home/lib/row-titles";
import { posterTile } from "@/features/home/lib/shelf-tiles";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";

/** One Plugin's row of posters under its heading, or nothing if it has no titles. */
export function RowShelf({ row, headingId }: { row: PluginHomeRow; headingId: string }) {
  const { data, hasNextPage, fetchNextPage, totalSize } = useHomeRowTitles(row);
  const titles = rowTitles(data.pages.flatMap((page) => page.titles));
  if (titles.length === 0 && !hasNextPage) {
    return null;
  }
  return (
    <RowFrame heading={row.heading} headingId={headingId}>
      <VirtualShelf
        tiles={titles.map((title) => ({
          key: rowTitleKey(title),
          tile: <RowPoster tile={rowTile(title)} />,
        }))}
        tile={posterTile}
        setSize={totalSize}
        next={{ has: hasNextPage, load: fetchNextPage }}
      />
    </RowFrame>
  );
}
