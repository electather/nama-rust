import type { LinkOptions } from "@tanstack/react-router";
import { CirclePlus } from "lucide-react";

import type { MovieHit } from "@/gen/nama/v1/request_pb";
import { shownRequestState } from "@/shared/components/request-state/request-states";
import type { Shown } from "@/shared/components/state-label";
import { titleStanding } from "@/shared/lib/request-standing";
import type { RowTitle } from "@/shared/lib/row-titles";
import { yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A shelf badge's state, with a `short` word to show when its label won't fit a poster's corner. */
export type TileBadgeShown = Shown & { short?: string };

/** A row's title as its shelf shows it: a poster that opens its page, maybe with a badge. */
export interface RowTile {
  title: string;
  year: string | undefined;
  posterUrl: string | undefined;
  posterThumbhash: Uint8Array | undefined;
  link: LinkOptions;
  /** Where a title missing from the Server stands; none for one you can watch. */
  badge: TileBadgeShown | undefined;
}

/** A missing title nobody has asked for. */
const notHere: TileBadgeShown = {
  label: "Not on this Server",
  short: "Not here",
  Icon: CirclePlus,
};

/** The tile of a row's `title`. */
export function rowTile(title: RowTitle): RowTile {
  const { value } = title;
  const art = {
    title: value.title,
    year: yearOfRowTitle(title),
    posterUrl: value.posterUrl,
    posterThumbhash: value.posterThumbhash,
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
 * Where a missing Movie stands on a shelf: its Request's state while it's on its way or Failed,
 * none once any of it is here, else `notHere`.
 */
function missingMovieBadge({ request, libraryId }: MovieHit): TileBadgeShown | undefined {
  const standing = titleStanding({ request, inLibrary: libraryId !== undefined });
  if (standing.kind === "inLibrary" || standing.kind === "partlyArrived") {
    return undefined;
  }
  if (standing.kind === "unknown" || !("request" in standing)) {
    return notHere;
  }
  return shownRequestState(standing.request.state);
}
