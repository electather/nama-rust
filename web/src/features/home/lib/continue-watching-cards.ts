import type { LinkOptions } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import type { BackdropCard } from "@/features/home/components/backdrop-card";
import type { Hiding } from "@/features/home/lib/hiding";
import type {
  ContinueWatchingEntry,
  ContinueWatchingEpisode,
  Movie,
} from "@/gen/nama/v1/browse_pb";
import { episodeName, metadataLine, timeLeft } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/** A Continue Watching entry: where it plays, resuming where you stopped, its card, and hiding it. */
export interface Card {
  link: LinkOptions;
  backdrop: ComponentProps<typeof BackdropCard>;
  hiding: Hiding;
}

/** The card of a Continue Watching entry, or none for one the Server left incomplete. */
export function entryCard({ libraryId, movieOrEpisode }: ContinueWatchingEntry): Card[] {
  if (movieOrEpisode.case === "movie") {
    return movieCard(libraryId, movieOrEpisode.value);
  }
  if (movieOrEpisode.case === "episode") {
    return episodeCard(libraryId, movieOrEpisode.value);
  }
  return [];
}

function movieCard(library: bigint, movie: Movie): Card[] {
  // Protobuf message fields are always optional; the Server sets it.
  if (!movie.id) {
    return [];
  }
  const libraryId = String(library);
  const movieId = titleParam(movie.id);
  return [
    {
      hiding: {
        key: `${libraryId}/${movieId}`,
        title: movie.title,
        libraryId: library,
        target: { case: "movieId", value: movie.id },
      },
      link: {
        to: "/libraries/$libraryId/movies/$movieId/play",
        params: { libraryId, movieId },
      },
      backdrop: {
        url: movie.backdropUrl,
        thumbhash: movie.backdropThumbhash,
        title: movie.title,
        logoUrl: movie.titleLogoUrl,
        detail: timeLeft(movie.watchState),
        watchState: movie.watchState,
      },
    },
  ];
}

/** An Episode shows its Show's Backdrop, saying which Episode it is, such as "S1, E3". */
function episodeCard(library: bigint, { show, episode }: ContinueWatchingEpisode): Card[] {
  // Protobuf message fields are always optional; the Server sets them.
  if (!show?.id || !episode) {
    return [];
  }
  const libraryId = String(library);
  const showId = titleParam(show.id);
  const season = String(episode.seasonNumber);
  const number = String(episode.episodeNumber);
  return [
    {
      hiding: {
        key: `${libraryId}/${showId}/${season}/${number}`,
        title: show.title,
        libraryId: library,
        target: { case: "showId", value: show.id },
      },
      link: {
        to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
        params: { libraryId, showId, season, episode: number },
      },
      backdrop: {
        url: show.backdropUrl,
        thumbhash: show.backdropThumbhash,
        title: show.title,
        logoUrl: show.titleLogoUrl,
        detail: metadataLine(
          episodeName(episode.seasonNumber, episode.episodeNumber, "short"),
          timeLeft(episode.watchState),
        ),
        watchState: episode.watchState,
      },
    },
  ];
}
