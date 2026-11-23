import type { LinkOptions } from "@tanstack/react-router";
import { CirclePlus } from "lucide-react";

import { type MovieHit, RequestState } from "@/gen/nama/v1/request_pb";
import { shownRequestState } from "@/shared/components/request-state/request-states";
import type { Shown } from "@/shared/components/state-label";
import type { RowTitle } from "@/shared/lib/row-titles";
import { titleParam } from "@/shared/lib/title-param";

/** A shelf badge's state, with a `short` word to show when its label won't fit a poster's corner. */
export type TileBadgeShown = Shown & { short?: string };

/** A row's title as its shelf shows it: a poster that opens its page, maybe with a badge. */
export interface RowTile {
  title: string;
  posterUrl: string | undefined;
  posterThumbhash: Uint8Array | undefined;
  link: LinkOptions;
  /** Where a title missing from the Server stands; none for one you can watch. */
  badge: TileBadgeShown | undefined;
}

/** A missing title nobody has asked for, or whose Request ended without it. */
const notHere: TileBadgeShown = {
  label: "Not on this Server",
  short: "Not here",
  Icon: CirclePlus,
};

/** The Request states a shelf badges with their own label; the rest read as `notHere` or none. */
const requested = new Set<RequestState>([
  RequestState.PENDING,
  RequestState.APPROVED,
  RequestState.PROCESSING,
]);
const arrived = new Set<RequestState>([RequestState.AVAILABLE, RequestState.PARTIALLY_AVAILABLE]);

/** The tile of a row's `title`. */
export function rowTile(title: RowTitle): RowTile {
  const { value } = title;
  const art = {
    title: value.title,
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

function missingMovieBadge(movie: MovieHit) {
  return movie.libraryId === undefined ? badgeFor(movie.request?.state) : undefined;
}

/** A Request's state on a shelf: its own label while asked for, none once it's here. */
function badgeFor(state: RequestState | undefined): TileBadgeShown | undefined {
  if (state === undefined) {
    return notHere;
  }
  if (arrived.has(state)) {
    return undefined;
  }
  return requested.has(state) ? shownRequestState(state) : notHere;
}
