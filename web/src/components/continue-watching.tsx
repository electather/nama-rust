import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { BackdropCard, BackdropGrid } from "@/components/backdrop-card";
import {
  HiddenNotice,
  HideMenu,
  type Hiding,
  useHideFromContinueWatching,
} from "@/components/hide-from-continue-watching";
import { posterLink } from "@/components/poster";
import {
  BrowseService,
  type ContinueWatchingEntry,
  type ContinueWatchingEpisode,
  type Movie,
} from "@/gen/nama/v1/browse_pb";
import { episodeName, metadataLine, timeLeft, titleParam } from "@/lib/browse";

/** Home's one page of Continue Watching: whole rows at one, two and three columns. */
export const continueWatchingRow = { page: { size: 6 } };

/**
 * What you've started and not played, and the next Episode of each Show you're watching, most
 * recent first, each with a menu to hide it. Nothing at all when there's none and nothing to undo.
 */
export function ContinueWatching() {
  const { data } = useSuspenseQuery(BrowseService.method.listContinueWatching, continueWatchingRow);
  const { lastHidden, hide, undo } = useHideFromContinueWatching();
  const cards = data.entries.flatMap(entryCard);
  if (cards.length === 0 && !lastHidden) {
    return null;
  }
  return (
    <section aria-labelledby="continue-watching" className="mt-10">
      <h2 id="continue-watching" className="font-medium">
        Continue Watching
      </h2>
      <HiddenNotice lastHidden={lastHidden} onUndo={undo} />
      {cards.length > 0 && (
        <BackdropGrid>
          {cards.map(({ link, backdrop, hiding }) => (
            <li key={hiding.key} className="relative">
              <Link {...link} className={posterLink}>
                <BackdropCard {...backdrop} />
              </Link>
              <HideMenu
                title={hiding.title}
                onHide={() => {
                  hide(hiding);
                }}
              />
            </li>
          ))}
        </BackdropGrid>
      )}
    </section>
  );
}

/** A Continue Watching entry: where it plays, resuming where you stopped, its card, and hiding it. */
interface Card {
  link: LinkOptions;
  backdrop: ComponentProps<typeof BackdropCard>;
  hiding: Hiding;
}

function entryCard({ libraryId, movieOrEpisode }: ContinueWatchingEntry): Card[] {
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
