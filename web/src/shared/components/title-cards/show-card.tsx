import { Link } from "@tanstack/react-router";
import { Check } from "lucide-react";

import type { ShowHit } from "@/gen/nama/v1/request_pb";
import { TitleStatus } from "@/gen/nama/v1/search_pb";
import { Poster } from "@/shared/components/poster/poster";
import { RequestPill } from "@/shared/components/request-state/request-pill";
import { StateLabel } from "@/shared/components/state-label";
import { posterLink } from "@/shared/lib/poster-classes";
import { titleParam } from "@/shared/lib/title-param";

/**
 * A Show found: its poster opens its page, in its Library if one has it, or else to pick Seasons,
 * over its Request's pill if it has a `status` of Requested.
 */
export function ShowCard({ show, status }: { show: ShowHit; status: TitleStatus }) {
  const showId = titleParam(show.id);
  const poster = (
    <Poster
      url={show.posterUrl}
      thumbhash={show.posterThumbhash}
      title={show.title}
      detail={show.year?.toString()}
    />
  );
  return (
    <li className="flex flex-col">
      {show.libraryId === undefined ? (
        <Link to="/shows/$showId" params={{ showId }} className={posterLink}>
          {poster}
        </Link>
      ) : (
        <Link
          to="/libraries/$libraryId/shows/$showId"
          params={{ libraryId: String(show.libraryId), showId }}
          className={posterLink}
        >
          {poster}
        </Link>
      )}
      {status === TitleStatus.REQUESTED && (
        <div className="mt-2 flex flex-col items-start gap-1">
          {/* Only its Followed Show is open: no Season Request to show the pill of. */}
          {show.request ? (
            <RequestPill request={show.request} />
          ) : (
            <StateLabel label="Requested" Icon={Check} />
          )}
        </div>
      )}
    </li>
  );
}
