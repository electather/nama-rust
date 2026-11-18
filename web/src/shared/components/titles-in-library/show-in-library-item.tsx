import { Link } from "@tanstack/react-router";

import type { ShowInLibrary } from "@/gen/nama/v1/browse_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";
import { titleParam } from "@/shared/lib/title-param";

/** A Show in a grid of posters, opening it in its Library. */
export function ShowInLibraryItem({ show }: { show: ShowInLibrary }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId/shows/$showId"
        params={{ libraryId: String(show.libraryId), showId: titleParam(show.id) }}
        className={posterLink}
      >
        <Poster url={show.posterUrl} thumbhash={show.posterThumbhash} title={show.title} />
      </Link>
    </li>
  );
}
