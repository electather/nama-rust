import type { LinkOptions } from "@tanstack/react-router";

import type { Hiding } from "@/features/home/lib/hiding";
import type { Marking } from "@/features/home/lib/marking";
import type {
  ContinueWatchingEntry,
  ContinueWatchingEpisode,
  Episode,
  Movie,
  Show,
} from "@/gen/nama/v1/browse_pb";
import type { ExternalId } from "@/gen/nama/v1/match_pb";
import type { WatchState } from "@/gen/nama/v1/watch_state_pb";
import {
  episodeName,
  metadataLine,
  remainingOf,
  runtimeOf,
  timeLeft,
} from "@/shared/lib/title-facts";
import { titleParam } from "@/shared/lib/title-param";
import { isStarted } from "@/shared/lib/watch-progress";

/**
 * A Continue Watching entry: where it plays, resuming where you stopped, its card, and marking it
 * watched or removing it.
 */
export interface Card {
  link: LinkOptions;
  backdrop: {
    url: string | undefined;
    thumbhash: Uint8Array | undefined;
    title: string;
    /** Its Title Logo, shown in place of its title, if it has one. */
    logoUrl: string | undefined;
    /** Which Episode, and how much is left, such as "S1, E3 · 33m left", as its link says it. */
    detail: string | undefined;
    /** `detail` as the card shows it, beside its bar, which says the rest: "S1, E3 · 33m". */
    shownDetail: string | undefined;
    watchState: WatchState | undefined;
  };
  /** Its Movie's or Show's page, which its menu opens. */
  page: { label: "Go to Movie" | "Go to Show"; link: LinkOptions };
  marking: Marking;
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
  const title = { key: `${libraryId}/${movieId}`, title: movie.title, libraryId: library };
  return [
    {
      page: {
        label: "Go to Movie",
        link: { to: "/libraries/$libraryId/movies/$movieId", params: { libraryId, movieId } },
      },
      marking: { ...title, target: { case: "movieId", value: movie.id } },
      hiding: { ...title, target: { case: "movieId", value: movie.id } },
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
        shownDetail: remainingOf(movie.watchState),
        watchState: movie.watchState,
      },
    },
  ];
}

/** An Episode shows its Show's Backdrop, saying which Episode it is, such as "S1, E3". */
function episodeCard(library: bigint, entry: ContinueWatchingEpisode): Card[] {
  if (!isComplete(entry)) {
    return [];
  }
  const { show, episode } = entry;
  const libraryId = String(library);
  const showId = titleParam(show.id);
  const season = String(episode.seasonNumber);
  const number = String(episode.episodeNumber);
  const which = episodeName(episode.seasonNumber, episode.episodeNumber, "short");
  const time = episodeTime(episode);
  const title = {
    key: `${libraryId}/${showId}/${season}/${number}`,
    title: show.title,
    libraryId: library,
  };
  return [
    {
      page: {
        label: "Go to Show",
        link: { to: "/libraries/$libraryId/shows/$showId", params: { libraryId, showId } },
      },
      marking: { ...title, target: { case: "episodeId", value: episode.id } },
      // Removing an Episode removes its Show, till you play any of its Episodes again.
      hiding: { ...title, target: { case: "showId", value: show.id } },
      link: {
        to: "/libraries/$libraryId/shows/$showId/seasons/$season/episodes/$episode/play",
        params: { libraryId, showId, season, episode: number },
      },
      backdrop: {
        url: show.backdropUrl,
        thumbhash: show.backdropThumbhash,
        title: show.title,
        logoUrl: show.titleLogoUrl,
        detail: metadataLine(which, time.spoken),
        shownDetail: metadataLine(which, time.shown),
        watchState: episode.watchState,
      },
    },
  ];
}

/** An Episode entry with its Show's and Episode's ids, as the Server always sends it. */
type CompleteEpisode = ContinueWatchingEpisode & {
  show: Show & { id: ExternalId };
  episode: Episode & { id: ExternalId };
};

/** Whether the Server set `entry`'s ids: protobuf message fields are always optional. */
function isComplete(entry: ContinueWatchingEpisode): entry is CompleteEpisode {
  return entry.show?.id !== undefined && entry.episode?.id !== undefined;
}

/**
 * An Episode's time as its card says it: what's left once you've started it, or else how long it
 * runs, as its link says it and as the card shows it beside its bar.
 */
function episodeTime({ watchState, runtimeMinutes }: Episode) {
  if (isStarted(watchState)) {
    return { spoken: timeLeft(watchState), shown: remainingOf(watchState) };
  }
  const runtime = runtimeOf(runtimeMinutes, "short");
  return { spoken: runtime, shown: runtime };
}
