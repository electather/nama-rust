import { useId } from "react";

import { boxSetTiles } from "@/features/title/lib/box-set-tiles";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterTile, tileGap, tileItem } from "@/shared/lib/tile-shapes";

/** A Box Set's Movies, in its order, under "Movies", as posters that wrap. */
export function BoxSetMovies({ boxSet }: { boxSet: BoxSet }) {
  const headingId = useId();
  const items = boxSetTiles(boxSet).map(({ key, ...tile }) => (
    <li key={key} {...tileItem(posterTile)}>
      <Poster {...tile} />
    </li>
  ));
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <h2 id={headingId} className="text-lg font-semibold">
        Movies
      </h2>
      <ul className={`flex flex-wrap ${tileGap.className}`}>{items}</ul>
    </section>
  );
}
