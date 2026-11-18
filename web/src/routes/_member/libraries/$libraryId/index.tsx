import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { LibraryPage } from "@/features/library/components/library-page";
import { firstUnmatchedPage } from "@/features/library/lib/first-unmatched-page";
import { firstPageOf, listingFromSearch } from "@/features/library/lib/title-listing";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { libraryFromParam, queryOrNotFound } from "@/shared/hooks/browse";
import { pagedQuery } from "@/shared/hooks/use-paged";
import { deletedFromSearch } from "@/shared/lib/deleted-search";

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
  const navigate = Route.useNavigate();
  return (
    <LibraryPage
      library={library}
      listing={listingFromSearch(search)}
      deleted={search.deleted}
      notTold={search.notTold ?? []}
      change={(changed) => {
        void navigate({ search: changed });
      }}
    />
  );
}
