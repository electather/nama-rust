import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, type LinkOptions } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { BackdropCard, BackdropGrid } from "@/components/backdrop-card";
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
 * recent first. Nothing at all when there's none.
 */
export function ContinueWatching() {
  const { data } = useSuspenseQuery(BrowseService.method.listContinueWatching, continueWatchingRow);
  if (data.entries.length === 0) return null;
  return (
    <section aria-labelledby="continue-watching" className="mt-10">
      <h2 id="continue-watching" className="font-medium">
        Continue Watching
      </h2>
      <BackdropGrid>
        {data.entries.flatMap(entryCard).map(({ key, link, backdrop }) => (
          <li key={key}>
            <Link {...link} className={posterLink}>
              <BackdropCard {...backdrop} />
            </Link>
          </li>
        ))}
      </BackdropGrid>
    </section>
  );
}

/** A Continue Watching entry: where it plays, resuming where you stopped, and its card. */
interface Card {
  key: string;
  link: LinkOptions;
  backdrop: ComponentProps<typeof BackdropCard>;
}

function entryCard({ libraryId, movieOrEpisode }: ContinueWatchingEntry): Card[] {
  const library = String(libraryId);
  if (movieOrEpisode.case === "movie") return [movieCard(library, movieOrEpisode.value)];
  if (movieOrEpisode.case === "episode") return episodeCard(library, movieOrEpisode.value);
  return [];
}

function movieCard(libraryId: string, movie: Movie): Card {
  const movieId = titleParam(movie.id);
  return {
    key: `${libraryId}/${movieId}`,
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
  };
}

/** An Episode shows its Show's Backdrop, saying which Episode it is, such as "S1, E3". */
function episodeCard(libraryId: string, { show, episode }: ContinueWatchingEpisode): Card[] {
  // Protobuf message fields are always optional; the Server sets both.
  if (!show || !episode) return [];
  const showId = titleParam(show.id);
  const season = String(episode.seasonNumber);
  const number = String(episode.episodeNumber);
  return [
    {
      key: `${libraryId}/${showId}/${season}/${number}`,
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
