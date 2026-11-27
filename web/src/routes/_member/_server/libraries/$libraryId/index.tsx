import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { LibraryPage } from "@/features/library/components/library-page";
import { firstUnmatchedPage } from "@/features/library/lib/first-unmatched-page";
import { firstBoxSetPage, viewFromSearch } from "@/features/library/lib/library-view";
import { firstPageOf, listingFromSearch } from "@/features/library/lib/title-listing";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";
import { libraryFromParam, libraryIdFromParam, queryOrNotFound } from "@/shared/hooks/browse";
import { pagedQuery } from "@/shared/hooks/use-paged";
import { deletedFromSearch } from "@/shared/lib/deleted-search";

export const Route = createFileRoute("/_member/_server/libraries/$libraryId/")({
  // The title whose page had the delete's result is gone, so the Fulfillers not told come here.
  validateSearch: (search: Record<string, unknown>) => ({
    ...deletedFromSearch(search),
    ...listingFromSearch(search),
    ...viewFromSearch(search),
  }),
  loaderDeps: ({ search }) => ({ ...listingFromSearch(search), ...viewFromSearch(search) }),
  loader: async ({ context, params, deps }) => {
    const { queryClient, transport } = context;
    const libraryId = libraryIdFromParam(params.libraryId);
    // Only the titles need the Library's kind, so these go out without waiting for the Libraries.
    const genres = queryOrNotFound(context, BrowseService.method.listGenres, { libraryId });
    const unmatched = queryClient.infiniteQuery({
      ...pagedQuery(BrowseService.method.listUnmatched, firstUnmatchedPage(libraryId), transport),
      staleTime: "static",
    });
    // Handled now, so a failure while the Libraries load isn't unhandled; awaited below.
    void Promise.allSettled([genres, unmatched]);
    const library = await libraryFromParam(context, params.libraryId);
    const boxSets =
      library.kind === LibraryKind.MOVIES && deps.view === "box-sets"
        ? queryClient.infiniteQuery({
            ...pagedQuery(BrowseService.method.listBoxSets, firstBoxSetPage(libraryId), transport),
            staleTime: "static",
          })
        : undefined;
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
    await Promise.all([titles, boxSets, genres, unmatched]);
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
      view={search.view ?? "movies"}
      deleted={search.deleted}
      notTold={search.notTold ?? []}
      change={(changed) => {
        void navigate({ search: changed });
      }}
      switchView={(view) => {
        // The Movies' sort and filter stay in the URL, so coming back to them finds them as left.
        const listing = listingFromSearch(search);
        void navigate({ search: view === "box-sets" ? { ...listing, view } : listing });
      }}
    />
  );
}
