import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, type LinkOptions, createFileRoute } from "@tanstack/react-router";
import type { ComponentProps } from "react";

import { LibraryCard } from "@/components/library-card";
import { Page } from "@/components/page";
import { Poster, PosterGrid, posterLink } from "@/components/poster";
import {
  BrowseService,
  type ContinueWatchingEntry,
  type ContinueWatchingEpisode,
  type Movie,
} from "@/gen/nama/v1/browse_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import { ServerService } from "@/gen/nama/v1/server_pb";
import { metadataLine, seasonName, titleParam, yearOf } from "@/lib/browse";

export const Route = createFileRoute("/_member/")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query({
        ...createQueryOptions(LibraryService.method.listLibraries, {}, { transport }),
        staleTime: "static",
      }),
      queryClient.query({
        ...createQueryOptions(BrowseService.method.listContinueWatching, {}, { transport }),
        staleTime: "static",
      }),
    ]),
  component: Home,
});

function Home() {
  // Fetched already, to know whether the Server needs setting up.
  const { data: server } = useSuspenseQuery(ServerService.method.getServerInfo, {});
  return (
    <Page heading={server.name}>
      <ContinueWatching />
      <section aria-labelledby="libraries" className="mt-10">
        <h2 id="libraries" className="font-medium">
          Libraries
        </h2>
        <Libraries />
      </section>
    </Page>
  );
}

/** What you've started and not played, most recent first. Nothing at all when there's none. */
function ContinueWatching() {
  const { data } = useSuspenseQuery(BrowseService.method.listContinueWatching, {});
  if (data.entries.length === 0) return null;
  return (
    <section aria-labelledby="continue-watching" className="mt-10">
      <h2 id="continue-watching" className="font-medium">
        Continue Watching
      </h2>
      <PosterGrid empty={null}>
        {data.entries.flatMap(entryCard).map(({ key, link, poster }) => (
          <li key={key}>
            <Link {...link} className={posterLink}>
              <Poster {...poster} />
            </Link>
          </li>
        ))}
      </PosterGrid>
    </section>
  );
}

/** A Continue Watching entry: where it plays, resuming where you stopped, and its poster. */
interface Card {
  key: string;
  link: LinkOptions;
  poster: ComponentProps<typeof Poster>;
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
    poster: {
      url: movie.posterUrl,
      thumbhash: movie.posterThumbhash,
      title: movie.title,
      detail: yearOf(movie.releaseDate),
      watchState: movie.watchState,
    },
  };
}

/** An Episode shows its Show's poster, saying which Episode it is. */
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
      poster: {
        url: show.posterUrl,
        thumbhash: show.posterThumbhash,
        title: show.title,
        detail: metadataLine(seasonName(episode.seasonNumber), `Episode ${number}`),
        watchState: episode.watchState,
      },
    },
  ];
}

function Libraries() {
  const { data } = useSuspenseQuery(LibraryService.method.listLibraries, {});
  if (data.libraries.length === 0) {
    return (
      <p className="mt-2 text-muted-foreground">
        This Server has no Libraries yet. Once one is added, it shows up here.
      </p>
    );
  }
  return (
    <ul className="mt-4 grid gap-x-4 gap-y-6 sm:grid-cols-2 lg:grid-cols-3">
      {data.libraries.map((library) => (
        <li key={String(library.id)}>
          <Link
            to="/libraries/$libraryId"
            params={{ libraryId: String(library.id) }}
            className={posterLink}
          >
            <LibraryCard library={library} />
          </Link>
        </li>
      ))}
    </ul>
  );
}
