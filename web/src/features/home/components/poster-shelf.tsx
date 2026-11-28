import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import type { ShelfTitles } from "@/features/home/lib/shelf-tiles";
import { Poster } from "@/shared/components/poster/poster";
import { rowTile } from "@/shared/components/poster/row-tiles";
import { rowTitleKey } from "@/shared/lib/row-titles";
import { posterTile } from "@/shared/lib/tile-shapes";

/**
 * A row's `titles` as posters, each opening its page, loading `next` as it nears its end.
 * `restoreId` is `VirtualShelf`'s.
 */
export function PosterShelf({
  titles,
  setSize,
  restoreId,
  next,
}: ShelfTitles & { restoreId: string }) {
  return (
    <VirtualShelf
      tiles={titles.map((title) => ({
        key: rowTitleKey(title),
        tile: <Poster {...rowTile(title)} />,
      }))}
      tile={posterTile}
      setSize={setSize}
      restoreId={restoreId}
      next={next}
    />
  );
}
