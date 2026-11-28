import type { LinkOptions } from "@tanstack/react-router";
import { CirclePlus } from "lucide-react";

import type { MovieHit } from "@/gen/nama/v1/request_pb";
import type { PosterData } from "@/shared/components/poster/poster";
import type { TileBadgeShown } from "@/shared/components/poster/tile-badge";
import { shownRequestState } from "@/shared/components/request-state/request-states";
import { titleStanding } from "@/shared/lib/request-standing";
import type { RowTitle } from "@/shared/lib/row-titles";
import { posterOf } from "@/shared/lib/server-image";
import { yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A missing title nobody has asked for. */
const notHere: TileBadgeShown = {
  label: "Not on this Server",
  short: "Not here",
  Icon: CirclePlus,
};

/** The poster of a row's `title`, badged with where it stands if it's missing from the Server. */
export function rowTile(title: RowTitle): PosterData {
  const { value } = title;
  const art = {
    title: value.title,
    detail: yearOfRowTitle(title),
    art: posterOf(value),
  };
  if (title.case === "missingMovie") {
    return { ...art, link: missingMovieLink(title.value), badge: missingMovieBadge(title.value) };
  }
  if (title.case === "missingShow") {
    return {
      ...art,
      link: { to: "/shows/$showId", params: { showId: titleParam(value.id) } },
      badge: notHere,
    };
  }
  const params = { libraryId: String(title.value.libraryId) };
  if (title.case === "movie") {
    return {
      ...art,
      link: {
        to: "/libraries/$libraryId/movies/$movieId",
        params: { ...params, movieId: titleParam(value.id) },
      },
      badge: undefined,
    };
  }
  return {
    ...art,
    link: {
      to: "/libraries/$libraryId/shows/$showId",
      params: { ...params, showId: titleParam(value.id) },
    },
    badge: undefined,
  };
}

function yearOfRowTitle(title: RowTitle) {
  if (title.case === "movie") {
    return yearOf(title.value.releaseDate);
  }
  if (title.case === "show") {
    return yearOf(title.value.firstAirDate);
  }
  return title.value.year?.toString();
}

/** A missing Movie opens in the Library that has it since, if one does. */
function missingMovieLink(movie: MovieHit): LinkOptions {
  const movieId = titleParam(movie.id);
  if (movie.libraryId === undefined) {
    return { to: "/movies/$movieId", params: { movieId } };
  }
  return {
    to: "/libraries/$libraryId/movies/$movieId",
    params: { libraryId: String(movie.libraryId), movieId },
  };
}

/**
 * Where a missing Movie stands on a shelf: its open Request's state, none once any of it is here,
 * else `notHere`.
 */
function missingMovieBadge({ request, libraryId }: MovieHit): TileBadgeShown | undefined {
  const standing = titleStanding({ request, inLibrary: libraryId !== undefined });
  if (standing.kind === "inLibrary" || standing.kind === "partlyArrived") {
    return undefined;
  }
  return "request" in standing ? shownRequestState(standing.request.state) : notHere;
}
