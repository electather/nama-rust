import { useId } from "react";

import { BoxSetHeading } from "@/features/title/components/box-set-heading";
import { BoxSetPoster } from "@/features/title/components/box-set-poster";
import { boxSetTiles } from "@/features/title/lib/box-set-tiles";
import type { BoxSet } from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { shelfBleed } from "@/shared/lib/shelf-bleed";

/**
 * A Movie's Box Set, under its name, which opens the Box Set's page: its Movies in one row that
 * scrolls sideways as the Similar titles row does, the Movie `current` marked. Nothing without one.
 */
export function BoxSetRow({
  boxSet,
  current,
}: {
  boxSet: BoxSet | undefined;
  current: ExternalId | undefined;
}) {
  const headingId = useId();
  if (!boxSet) {
    return null;
  }
  return (
    <section aria-labelledby={headingId} className="flex flex-col gap-4">
      <BoxSetHeading boxSet={boxSet} id={headingId} />
      {/* Overscroll stays on the row, so a swipe at its end doesn't go back a page. */}
      <ul
        className={`flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-5 overflow-x-auto overscroll-x-contain ${shelfBleed}`}
      >
        {boxSetTiles(boxSet, current).map((tile) => (
          <BoxSetPoster key={tile.key} tile={tile} />
        ))}
      </ul>
    </section>
  );
}
