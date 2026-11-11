import { useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { MayDownloadAgain } from "@/components/delete-movie";
import { Page } from "@/components/page";
import { NotFound } from "@/components/route-error";
import { Poster, PosterGrid, posterLink } from "@/components/poster";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { libraryFromParam, queryOrNotFound, titleParam, yearOf } from "@/lib/browse";

export const Route = createFileRoute("/_member/libraries/$libraryId/")({
  // The Movie whose page had the delete's result is gone, so the Fulfillers not told come here.
  validateSearch: (
    search: Record<string, unknown>,
  ): { deleted?: string | undefined; notTold?: string[] | undefined } => {
    const { deleted, notTold } = { deleted: search["deleted"], notTold: search["notTold"] };
    // Set even when unknown: the router keeps the raw value under any key this leaves out.
    const unknown = { deleted: undefined, notTold: undefined };
    return typeof deleted === "string" && isNames(notTold) ? { deleted, notTold } : unknown;
  },
  loader: async ({ context, params }) => {
    const library = await libraryFromParam(context, params.libraryId);
    const input = { libraryId: library.id };
    await (library.kind === LibraryKind.SHOWS
      ? queryOrNotFound(context, BrowseService.method.listShows, input)
      : queryOrNotFound(context, BrowseService.method.listMovies, input));
    return library;
  },
  notFoundComponent: NotFound,
  component: Library,
});

function Library() {
  const library = Route.useLoaderData();
  const { deleted, notTold } = Route.useSearch();
  return (
    <Page heading={library.name}>
      {deleted !== undefined && (
        <div className="mt-2">
          <MayDownloadAgain title={deleted} fulfillers={notTold ?? []} />
        </div>
      )}
      {library.kind === LibraryKind.SHOWS ? (
        <Shows libraryId={library.id} />
      ) : (
        <Movies libraryId={library.id} />
      )}
    </Page>
  );
}

function Movies({ libraryId }: { libraryId: bigint }) {
  const { data } = useSuspenseQuery(BrowseService.method.listMovies, { libraryId });
  return (
    <PosterGrid empty={<EmptyLibrary titles="Movies" />}>
      {data.movies.map((movie) => (
        <li key={titleParam(movie.id)}>
          <Link
            to="/libraries/$libraryId/movies/$movieId"
            params={{ libraryId: String(libraryId), movieId: titleParam(movie.id) }}
            className={posterLink}
          >
            <Poster
              url={movie.posterUrl}
              title={movie.title}
              detail={yearOf(movie.releaseDate)}
              watchState={movie.watchState}
            />
          </Link>
        </li>
      ))}
    </PosterGrid>
  );
}

function Shows({ libraryId }: { libraryId: bigint }) {
  const { data } = useSuspenseQuery(BrowseService.method.listShows, { libraryId });
  return (
    <PosterGrid empty={<EmptyLibrary titles="Shows" />}>
      {data.shows.map((show) => (
        <li key={titleParam(show.id)}>
          <Link
            to="/libraries/$libraryId/shows/$showId"
            params={{ libraryId: String(libraryId), showId: titleParam(show.id) }}
            className={posterLink}
          >
            <Poster url={show.posterUrl} title={show.title} />
          </Link>
        </li>
      ))}
    </PosterGrid>
  );
}

function isNames(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((name) => typeof name === "string");
}

function EmptyLibrary({ titles }: { titles: "Movies" | "Shows" }) {
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
