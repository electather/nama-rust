import { useQuery } from "@connectrpc/connect-query";
import { keepPreviousData } from "@tanstack/react-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Page } from "@/components/page";
import { Poster, PosterGrid, posterLink } from "@/components/poster";
import {
  BrowseService,
  type EpisodeInLibrary,
  type MovieInLibrary,
  type SearchLibrariesResponse,
  type ShowInLibrary,
} from "@/gen/nama/v1/browse_pb";
import { metadataLine, seasonName, titleParam, yearOf } from "@/lib/browse";

/** What the nav's search box found in the signed-in User's Libraries for `?query=`. */
export const Route = createFileRoute("/_member/search")({
  validateSearch: (search): { query?: string | undefined } => {
    // The router parses `?query=1917` as JSON, so a number is words too.
    const raw = search["query"];
    const query = typeof raw === "string" || typeof raw === "number" ? String(raw).trim() : "";
    // Set even when blank: the router keeps the raw value under any key this leaves out.
    return { query: query === "" ? undefined : query };
  },
  component: Search,
});

function Search() {
  const { query } = Route.useSearch();
  return (
    <Page heading={query === undefined ? "Search" : `Results for “${query}”`}>
      {query === undefined ? (
        <p className="mt-8 text-muted-foreground">
          Type in the search box to find a Movie, Show or Episode in your Libraries.
        </p>
      ) : (
        <Results query={query} />
      )}
    </Page>
  );
}

function Results({ query }: { query: string }) {
  // The last results stay up while the next keystroke's are on their way, so the page doesn't flash.
  const search = useQuery(
    BrowseService.method.searchLibraries,
    { query },
    { placeholderData: keepPreviousData },
  );
  if (search.isError) {
    return (
      <p role="alert" className="mt-8 text-muted-foreground">
        Couldn’t search your Libraries. Try again in a moment.
      </p>
    );
  }
  if (!search.data) return <p className="mt-8 text-muted-foreground">Searching…</p>;
  return <Found query={query} found={search.data} />;
}

/** What a search for `query` found, grouped by kind, or that it found nothing. */
function Found({ query, found }: { query: string; found: SearchLibrariesResponse }) {
  const { movies, shows, episodes } = found;
  if (movies.length + shows.length + episodes.length === 0) {
    return <p className="mt-8 text-muted-foreground">Nothing found for “{query}”.</p>;
  }
  // A Movie, Show or Episode in two Libraries is found in each, so keys name the Library too.
  return (
    <>
      <FoundSection heading="Movies" count={movies.length}>
        <PosterGrid empty={null}>
          {movies.map((movie) => (
            <MovieFound key={`${movie.libraryId}/${titleParam(movie.id)}`} movie={movie} />
          ))}
        </PosterGrid>
      </FoundSection>
      <FoundSection heading="Shows" count={shows.length}>
        <PosterGrid empty={null}>
          {shows.map((show) => (
            <ShowFound key={`${show.libraryId}/${titleParam(show.id)}`} show={show} />
          ))}
        </PosterGrid>
      </FoundSection>
      <FoundSection heading="Episodes" count={episodes.length}>
        <ul className="-mx-3 mt-4 max-w-3xl">
          {episodes.map((episode) => (
            <EpisodeFound
              key={`${episode.libraryId}/${titleParam(episode.id)}`}
              episode={episode}
            />
          ))}
        </ul>
      </FoundSection>
    </>
  );
}

/** The titles of one kind found, under their `heading`, or nothing when `count` is none. */
function FoundSection({
  heading,
  count,
  children,
}: {
  heading: string;
  count: number;
  children: ReactNode;
}) {
  if (count === 0) return null;
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {children}
    </section>
  );
}

function MovieFound({ movie }: { movie: MovieInLibrary }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId/movies/$movieId"
        params={{ libraryId: String(movie.libraryId), movieId: titleParam(movie.id) }}
        className={posterLink}
      >
        <Poster
          url={movie.posterUrl}
          thumbhash={movie.posterThumbhash}
          title={movie.title}
          detail={yearOf(movie.releaseDate)}
        />
      </Link>
    </li>
  );
}

function ShowFound({ show }: { show: ShowInLibrary }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId/shows/$showId"
        params={{ libraryId: String(show.libraryId), showId: titleParam(show.id) }}
        className={posterLink}
      >
        <Poster url={show.posterUrl} thumbhash={show.posterThumbhash} title={show.title} />
      </Link>
    </li>
  );
}

/** An Episode, opening its Show at its Season. Styled as the Show page's Episode rows. */
function EpisodeFound({ episode }: { episode: EpisodeInLibrary }) {
  return (
    <li className="border-b last:border-b-0">
      <Link
        to="/libraries/$libraryId/shows/$showId"
        params={{ libraryId: String(episode.libraryId), showId: titleParam(episode.showId) }}
        search={{ season: episode.seasonNumber }}
        className="block rounded-lg px-3 py-4 outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="font-medium">
          {episode.episodeNumber}. {episode.title}
        </span>
        <span className="mt-0.5 block text-sm text-muted-foreground">
          {metadataLine(episode.showTitle, seasonName(episode.seasonNumber))}
        </span>
      </Link>
    </li>
  );
}
