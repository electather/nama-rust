import { createFileRoute } from "@tanstack/react-router";

import { MayDownloadAgain } from "@/components/delete-media";
import { LibraryMovies, LibraryShows, type TitlesProps } from "@/components/library-titles";
import { NotMatched, firstUnmatchedPage } from "@/components/not-matched";
import { NotFound } from "@/components/route-error";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { deletedFromSearch, libraryFromParam, queryOrNotFound } from "@/lib/browse";
// oxlint-disable-next-line import/max-dependencies -- the Library page puts together its titles, their filters, its Unmatched Items and a delete's notice
import { firstPageOf, listingFromSearch } from "@/lib/title-listing";
import { Page } from "@/shared/components/page";
import { pagedQuery } from "@/shared/hooks/use-paged";

export const Route = createFileRoute("/_member/libraries/$libraryId/")({
  // The title whose page had the delete's result is gone, so the Fulfillers not told come here.
  validateSearch: (search: Record<string, unknown>) => ({
    ...deletedFromSearch(search),
    ...listingFromSearch(search),
  }),
  loaderDeps: ({ search }) => listingFromSearch(search),
  loader: async ({ context, params, deps }) => {
    const library = await libraryFromParam(context, params.libraryId);
    const { queryClient, transport } = context;
    const libraryId = library.id;
    const titles =
      library.kind === LibraryKind.SHOWS
        ? queryClient.infiniteQuery({
            ...pagedQuery(
              BrowseService.method.listShows,
              { libraryId, ...firstPageOf(deps, "shows") },
              transport,
            ),
            staleTime: "static",
          })
        : queryClient.infiniteQuery({
            ...pagedQuery(
              BrowseService.method.listMovies,
              { libraryId, ...firstPageOf(deps, "movies") },
              transport,
            ),
            staleTime: "static",
          });
    const unmatched = queryClient.infiniteQuery({
      ...pagedQuery(BrowseService.method.listUnmatched, firstUnmatchedPage(libraryId), transport),
      staleTime: "static",
    });
    await Promise.all([
      titles,
      queryOrNotFound(context, BrowseService.method.listGenres, { libraryId }),
      unmatched,
    ]);
    return library;
  },
  notFoundComponent: NotFound,
  component: Library,
});

function Library() {
  const library = Route.useLoaderData();
  const search = Route.useSearch();
  const { deleted, notTold = [] } = search;
  const navigate = Route.useNavigate();
  const titles: TitlesProps = {
    libraryId: library.id,
    listing: listingFromSearch(search),
    change: (changed) => {
      void navigate({ search: changed });
    },
  };
  return (
    <Page heading={library.name}>
      {deleted !== undefined && (
        <div className="mt-2">
          <MayDownloadAgain title={deleted} fulfillers={notTold} />
        </div>
      )}
      {library.kind === LibraryKind.SHOWS ? (
        <LibraryShows {...titles} />
      ) : (
        <LibraryMovies {...titles} />
      )}
      <NotMatched libraryId={library.id} />
    </Page>
  );
}
