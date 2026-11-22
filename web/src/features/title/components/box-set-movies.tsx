import { useId } from "react";

import { BoxSetPoster } from "@/features/title/components/box-set-poster";
import { boxSetTiles } from "@/features/title/lib/box-set-tiles";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";

/** A Box Set's Movies, in its order, under "Movies", as posters that wrap. */
export function BoxSetMovies({ boxSet }: { boxSet: BoxSet }) {
  const headingId = useId();
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <h2 id={headingId} className="text-lg font-semibold">
        Movies
      </h2>
      <ul className="flex flex-wrap gap-5">
        {boxSetTiles(boxSet).map((tile) => (
          <BoxSetPoster key={tile.key} tile={tile} />
        ))}
      </ul>
    </section>
  );
}
