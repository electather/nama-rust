import { useId } from "react";

import { OnServer } from "@/features/home/components/on-server";
import { RowFrame } from "@/features/home/components/row-frame";
import { VirtualShelf } from "@/features/home/components/virtual-shelf";
import { type TitleOn, onLinkedServer } from "@/features/home/lib/hub-titles";
import { posterTile } from "@/features/home/lib/shelf-tiles";
import { RowPoster } from "@/shared/components/row-poster/row-poster";
import { rowTile } from "@/shared/components/row-poster/row-tiles";
import { rowTitleKey } from "@/shared/lib/row-titles";

/** A row at the Hub: `titles` as posters, each opening on its Server, or nothing without any. */
export function HubTitlesRow({ heading, titles }: { heading: string; titles: TitleOn[] }) {
  const headingId = useId();
  if (titles.length === 0) {
    return null;
  }
  const tiles = titles.map(({ server, transport, title }) => {
    const tile = rowTile(title);
    return {
      key: `${server.id}/${rowTitleKey(title)}`,
      tile: (
        <OnServer transport={transport}>
          <RowPoster tile={{ ...tile, link: onLinkedServer(tile.link, server) }} />
        </OnServer>
      ),
    };
  });
  return (
    <RowFrame heading={heading} headingId={headingId}>
      <VirtualShelf tiles={tiles} tile={posterTile} setSize={tiles.length} />
    </RowFrame>
  );
}
