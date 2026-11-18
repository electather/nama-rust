import { Link } from "@tanstack/react-router";

import { listsOnlySeason } from "@/features/title/lib/season-to-show";
import type { Season } from "@/gen/nama/v1/browse_pb";
import { seasonName } from "@/shared/lib/title-facts";

/**
 * Links to the Show's Seasons, one shown at a time. None while the page lists a Show's only Season
 * besides Specials, with the Specials below; with only Specials, it names them.
 */
export function Seasons({ seasons, current }: { seasons: Season[]; current: number }) {
  if (listsOnlySeason(seasons, current)) {
    return null;
  }
  return (
    // Pulled out by the links' own padding, so the first lines up with the title.
    <nav aria-label="Seasons" className="-ms-3 flex flex-1 flex-wrap gap-1">
      {seasons.map(({ number }) => (
        <Link
          key={number}
          from="/libraries/$libraryId/shows/$showId/"
          search={{ season: number }}
          aria-current={number === current ? "page" : undefined}
          className="rounded-lg px-3 py-2 text-sm font-medium text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 aria-[current=page]:bg-muted aria-[current=page]:text-foreground"
        >
          {seasonName(number)}
        </Link>
      ))}
    </nav>
  );
}
