import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { type ShelfTitles, posterTile } from "@/features/home/lib/shelf-tiles";
import { RowPoster } from "@/shared/components/row-poster/row-poster";
import { rowTile } from "@/shared/components/row-poster/row-tiles";
import { rowTitleKey } from "@/shared/lib/row-titles";

/** A row's `titles` as posters, each opening its page, loading `next` as it nears its end. */
export function PosterShelf({ titles, setSize, next }: ShelfTitles) {
  return (
    <VirtualShelf
      tiles={titles.map((title) => ({
        key: rowTitleKey(title),
        tile: <RowPoster tile={rowTile(title)} />,
      }))}
      tile={posterTile}
      setSize={setSize}
      next={next}
    />
  );
}
