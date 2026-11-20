import { Link } from "@tanstack/react-router";

import type { ShowHit } from "@/gen/nama/v1/request_pb";
import { Poster } from "@/shared/components/poster/poster";
import { posterLink } from "@/shared/lib/poster-classes";
import { titleParam } from "@/shared/lib/title-param";

/** A Show found: its poster opens its page, to pick Seasons. */
export function ShowCard({ show }: { show: ShowHit }) {
  return (
    <li>
      <Link to="/shows/$showId" params={{ showId: titleParam(show.id) }} className={posterLink}>
        <Poster
          url={show.posterUrl}
          thumbhash={show.posterThumbhash}
          title={show.title}
          detail={show.year?.toString()}
        />
      </Link>
    </li>
  );
}
