import { useId } from "react";

import { RowFrame } from "@/features/home/components/row-frame";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import type { TitleOn } from "@/features/home/lib/hub-titles";
import { OnServer } from "@/shared/components/on-server";
import { RowPoster } from "@/shared/components/row-poster/row-poster";
import { rowTile } from "@/shared/components/row-poster/row-tiles";
import { rowTitleKey } from "@/shared/lib/row-titles";
import { posterTile } from "@/shared/lib/tile-shapes";
import { linkOn } from "@/shared/lib/title-server";

/** A row at the Hub: `titles` as posters, each opening on its Server, or nothing without any. */
export function HubTitlesRow({ heading, titles }: { heading: string; titles: TitleOn[] }) {
  const headingId = useId();
  if (titles.length === 0) {
    return null;
  }
  const tiles = titles.map((on) => {
    const tile = rowTile(on.title);
    return {
      key: `${on.server.id}/${rowTitleKey(on.title)}`,
      tile: (
        <OnServer on={on}>
          <RowPoster tile={{ ...tile, link: linkOn(tile.link, on) }} />
        </OnServer>
      ),
    };
  });
  return (
    <RowFrame heading={heading} headingId={headingId}>
      <VirtualShelf
        tiles={tiles}
        tile={posterTile}
        setSize={tiles.length}
        restoreId={`row/${heading}`}
      />
    </RowFrame>
  );
}
