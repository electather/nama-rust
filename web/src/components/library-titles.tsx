import { create } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { Poster, posterLink } from "@/components/poster";
import { ShowMore } from "@/components/show-more";
import { TitleListControls } from "@/components/title-list-controls";
import { VirtualPosterGrid } from "@/components/virtual-poster-grid";
import { BrowseService, type ShowSummary } from "@/gen/nama/v1/browse_pb";
import { WatchStateSchema } from "@/gen/nama/v1/watch_state_pb";
// oxlint-disable-next-line import/max-dependencies -- a Library's grid puts together its pages, its sort and filter bar and its posters
import { titleParam, yearOf } from "@/lib/browse";
import { usePaged } from "@/lib/paged";
import { type TitleListing, firstPageOf, isFiltered } from "@/lib/title-listing";

/** What a Library's grid lists, and what `change` shows it as when the User sorts or filters. */
export interface TitlesProps {
  libraryId: bigint;
  listing: TitleListing;
  change: (listing: TitleListing) => void;
}

/** A Movies Library's Movies, sorted and filtered as `listing` says. */
export function LibraryMovies({ libraryId, listing, change }: TitlesProps) {
  const paged = usePaged(BrowseService.method.listMovies, {
    libraryId,
    ...firstPageOf(listing, "movies"),
  });
  const posters = paged.data.pages
    .flatMap((page) => page.movies)
    .map((movie) => ({
      key: titleParam(movie.id),
      poster: (
        <Link
          to="/libraries/$libraryId/movies/$movieId"
          params={{ libraryId: String(libraryId), movieId: titleParam(movie.id) }}
          className={posterLink}
        >
          <Poster
            url={movie.posterUrl}
            thumbhash={movie.posterThumbhash}
            title={movie.title}
            detail={yearOf(movie.releaseDate)}
            watchState={movie.watchState}
          />
        </Link>
      ),
    }));
  return (
    <Titles
      titles={{ one: "Movie", many: "Movies" }}
      {...{ libraryId, listing, change, paged, posters }}
    />
  );
}

/** A Shows Library's Shows, sorted and filtered as `listing` says. */
export function LibraryShows({ libraryId, listing, change }: TitlesProps) {
  const paged = usePaged(BrowseService.method.listShows, {
    libraryId,
    ...firstPageOf(listing, "shows"),
  });
  const posters = paged.data.pages
    .flatMap((page) => page.shows)
    .map((show) => ({
      key: titleParam(show.id),
      poster: (
        <Link
          to="/libraries/$libraryId/shows/$showId"
          params={{ libraryId: String(libraryId), showId: titleParam(show.id) }}
          className={posterLink}
        >
          <Poster
            url={show.posterUrl}
            thumbhash={show.posterThumbhash}
            title={show.title}
            detail={yearOf(show.firstAirDate)}
            watchState={episodesWatched(show)}
          />
        </Link>
      ),
    }));
  return (
    <Titles
      titles={{ one: "Show", many: "Shows" }}
      {...{ libraryId, listing, change, paged, posters }}
    />
  );
}

/**
 * Your Episodes of `show` played, as a Watch State the poster shows: its bar is the share played
 * and its mark says you've played them all.
 */
function episodesWatched({ episodeCount, playedEpisodeCount }: ShowSummary) {
  return create(WatchStateSchema, {
    positionMs: BigInt(playedEpisodeCount),
    durationMs: BigInt(episodeCount),
    played: episodeCount > 0 && playedEpisodeCount === episodeCount,
  });
}

/**
 * A Library's titles so far: how many there are, the bar to sort and filter them, and their
 * posters, loading the next page as the grid's end scrolls into view. An empty Library says so
 * instead.
 */
function Titles({ libraryId, listing, change, ...grid }: TitlesProps & GridProps) {
  const { data: genres } = useSuspenseQuery(BrowseService.method.listGenres, { libraryId });
  if (grid.posters.length === 0 && !isFiltered(listing)) {
    return <EmptyLibrary titles={grid.titles.many} />;
  }
  return (
    <>
      <TitleCount count={grid.paged.totalSize} titles={grid.titles} />
      <TitleListControls listing={listing} genres={genres.genres} change={change} />
      <TitleGrid {...grid} />
    </>
  );
}

/** A Library's posters so far, as `usePaged` read them, with what they're called. */
interface GridProps {
  titles: { one: string; many: string };
  paged: Pick<ReturnType<typeof usePaged>, "hasNextPage" | "fetchNextPage" | "totalSize"> & {
    data: { pages: unknown[] };
  };
  posters: { key: string; poster: ReactNode }[];
}

/** The posters, and Show more under them while a page follows, or that none match the filters. */
function TitleGrid({ titles, paged, posters }: GridProps) {
  if (posters.length === 0) {
    return <p className="mt-8 text-muted-foreground">No {titles.many} match these filters.</p>;
  }
  return (
    <>
      <VirtualPosterGrid posters={posters} setSize={paged.totalSize} />
      {paged.hasNextPage && (
        <ShowMore load={paged.fetchNextPage} loaded={paged.data.pages.length} />
      )}
    </>
  );
}

/** "1 Movie", "1,000 Movies": how many titles the listing has, once the first page has said. */
function TitleCount({
  count,
  titles,
}: {
  count: number | undefined;
  titles: { one: string; many: string };
}) {
  if (count === undefined) {
    return null;
  }
  return (
    <p className="mt-1 text-sm text-muted-foreground">
      {count.toLocaleString()} {count === 1 ? titles.one : titles.many}
    </p>
  );
}

function EmptyLibrary({ titles }: { titles: string }) {
  return (
    <div className="mt-8">
      <p className="font-medium">Nothing in this Library yet</p>
      <p className="mt-1 max-w-prose text-muted-foreground">
        Its {titles} show up here once the Server has scanned its folders and Matched what they
        hold.
      </p>
    </div>
  );
}
