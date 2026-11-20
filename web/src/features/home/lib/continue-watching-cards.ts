import type { MessageInitShape } from "@bufbuild/protobuf";
import type { LinkOptions } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import type { BackdropCard } from "@/features/home/components/backdrop-card";
import type {
  ContinueWatchingEntry,
  ContinueWatchingEpisode,
  Episode,
  Movie,
  Show,
} from "@/gen/nama/v1/browse_pb";
import type { SetPlayedRequestSchema } from "@/gen/nama/v1/watch_state_pb";
import { episodeName, metadataLine, timeLeft } from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";

/**
 * A Continue Watching entry, keyed as the row lists it: where it plays, resuming where you stopped,
 * its card, and what marking its Movie or Episode watched asks.
 */
export interface Card {
  key: string;
  link: LinkOptions;
  backdrop: ComponentProps<typeof BackdropCard>;
  markWatched: MessageInitShape<typeof SetPlayedRequestSchema>;
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
      key: `${libraryId}/${movieId}`,
      markWatched: {
        libraryId: library,
        target: { case: "movieId", value: movie.id },
        played: true,
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

// Protobuf message fields are always optional; the Server sets them.
function episodeCard(library: bigint, { show, episode }: ContinueWatchingEpisode): Card[] {
  return show && episode ? episodeCardOf(library, show, episode) : [];
}

/** An Episode shows its Show's Backdrop, saying which Episode it is, such as "S1, E3". */
function episodeCardOf(library: bigint, show: Show, episode: Episode): Card[] {
  if (!show.id || !episode.id) {
    return [];
  }
  const libraryId = String(library);
  const showId = titleParam(show.id);
  const season = String(episode.seasonNumber);
  const number = String(episode.episodeNumber);
  return [
    {
      key: `${libraryId}/${showId}/${season}/${number}`,
      markWatched: {
        libraryId: library,
        target: { case: "episodeId", value: episode.id },
        played: true,
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
