import type { LinkOptions } from "@tanstack/react-router";

import type { Episode, FeaturedTitle, Movie, Show } from "@/gen/nama/v1/browse_pb";
import { type ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import { metadataLine, runtimeOf, yearOf } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * What a Featured panel's one action is: playing, or resuming, what `state` says, or, with
 * nothing to play, More info.
 */
export type PanelAction =
  | { kind: "play"; link: LinkOptions; state: WatchState | undefined }
  | { kind: "info"; link: LinkOptions };

/** A Featured title as its panel shows it. */
export interface Panel {
  key: string;
  title: string;
  /** Its Title Logo, shown in place of its title, if it has one. */
  logoUrl: string | undefined;
  backdropUrl: string | undefined;
  backdropThumbhash: Uint8Array | undefined;
  rating: string | undefined;
  facts: string;
  overview: string | undefined;
  action: PanelAction;
  /** Its page, which the open panel opens when clicked. */
  page: LinkOptions;
  watchlist: { kind: TitleKind; id: ExternalId; on: boolean };
}

/** The panels of your Featured, leaving out any title the Server left incomplete. */
export function featuredPanels(titles: FeaturedTitle[]): Panel[] {
  return titles.flatMap((title) => panelsOf(title));
}

function panelsOf({ libraryId, movieOrShow, nextEpisode }: FeaturedTitle): Panel[] {
  const library = String(libraryId);
  if (movieOrShow.case === "movie") {
    return moviePanel(library, movieOrShow.value);
  }
  if (movieOrShow.case === "show") {
    return showPanel(library, movieOrShow.value, nextEpisode);
  }
  return [];
}

function moviePanel(libraryId: string, movie: Movie): Panel[] {
  // Protobuf message fields are always optional; the Server sets it.
  if (!movie.id) {
    return [];
  }
  const { id } = movie;
  const movieId = titleParam(id);
  const link: LinkOptions = {
    to: "/libraries/$libraryId/movies/$movieId/play",
    params: { libraryId, movieId },
  };
  return [
    {
      ...shared(movie, `${libraryId}/${movieId}`),
      facts: metadataLine(
        yearOf(movie.releaseDate),
        runtimeOf(movie.runtimeMinutes, "short"),
        genres(movie.genres),
      ),
      action: { kind: "play", link, state: movie.watchState },
      page: { to: "/libraries/$libraryId/movies/$movieId", params: { libraryId, movieId } },
      watchlist: { kind: TitleKind.MOVIE, id, on: movie.onWatchlist },
    },
  ];
}

function showPanel(libraryId: string, show: Show, next: Episode | undefined): Panel[] {
  if (!show.id) {
    return [];
  }
  const { id } = show;
  const showId = titleParam(id);
  return [
    {
      ...shared(show, `${libraryId}/${showId}`),
      facts: metadataLine(
        yearOf(show.firstAirDate),
        seasons(show.seasonCount),
        genres(show.genres),
      ),
      action: next ? episodeAction({ libraryId, showId }, next) : showInfo(libraryId, showId),
      page: showPage(libraryId, showId),
      watchlist: { kind: TitleKind.SHOW, id, on: show.onWatchlist },
    },
  ];
}

/** What a Movie and a Show share on a panel. */
function shared(title: Movie | Show, key: string) {
  return {
    key,
    title: title.title,
    logoUrl: title.titleLogoUrl,
    backdropUrl: title.backdropUrl,
    backdropThumbhash: title.backdropThumbhash,
    rating: title.contentRating,
    overview: title.overview,
  };
}

function episodeAction(show: { libraryId: string; showId: string }, next: Episode): PanelAction {
  const season = String(next.seasonNumber);
  const episode = String(next.episodeNumber);
  return {
    kind: "play",
    state: next.watchState,
    link: {
      to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
      params: { ...show, season, episode },
    },
  };
}

/** A Show you've played every Episode of opens its page instead. */
function showInfo(libraryId: string, showId: string): PanelAction {
  return { kind: "info", link: showPage(libraryId, showId) };
}

function showPage(libraryId: string, showId: string): LinkOptions {
  return { to: "/libraries/$libraryId/shows/$showId", params: { libraryId, showId } };
}

/** The first two genres, as the artboard's facts line names them. */
function genres(names: string[]) {
  return names.length > 0 ? names.slice(0, 2).join(", ") : undefined;
}

function seasons(count: number) {
  if (count === 1) {
    return "1 Season";
  }
  return count > 1 ? `${count} Seasons` : undefined;
}
