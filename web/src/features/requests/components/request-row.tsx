import { Link } from "@tanstack/react-router";

import { RequestEntry } from "@/features/requests/components/request-entry";
import { useTitleLinkSearch } from "@/features/requests/hooks/use-title-link-search";
import type { Request } from "@/gen/nama/v1/request_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A Movie's Request, its title a link to the Movie's page. */
export function RequestRow({ request }: { request: Request }) {
  const search = useTitleLinkSearch();
  const { movieOrSeason } = request;
  const movie = movieOrSeason.case === "movie" ? movieOrSeason.value : undefined;
  const title = (
    <Link
      to="/movies/$movieId"
      search={search}
      params={{ movieId: titleParam(movie?.id) }}
      className="truncate text-sm font-medium underline-offset-4 hover:underline"
    >
      {movie?.title}
    </Link>
  );
  return (
    <li className="flex py-2.5 not-first:border-t">
      <RequestEntry request={request} title={title} />
    </li>
  );
}
