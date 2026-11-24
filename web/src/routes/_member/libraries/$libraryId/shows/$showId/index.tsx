import { createQueryOptions } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { ShowPage } from "@/features/title/components/show-page";
import { TitlePending } from "@/features/title/components/title-pending";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { externalIdFromParam, libraryFromParam, queryOrNotFound } from "@/shared/hooks/browse";
import { deletedFromSearch, type DeletedSearch } from "@/shared/lib/deleted-search";

export const Route = createFileRoute("/_member/libraries/$libraryId/shows/$showId/")({
  validateSearch: (search): { season?: number } & DeletedSearch => {
    const season = Number(search["season"]);
    const deleted = deletedFromSearch(search);
    return Number.isInteger(season) && season >= 0 ? { season, ...deleted } : deleted;
  },
  loaderDeps: ({ search }) => ({ season: search.season }),
  loader: async ({ context, params, deps }) => {
    const library = await libraryFromParam(context, params.libraryId);
    const show = { libraryId: library.id, showId: externalIdFromParam(params.showId) };
    const { queryClient, transport } = context;
    await Promise.all([
      queryOrNotFound(context, BrowseService.method.getShow, show),
      // Every Season's in one request, as the page shows them all in one row.
      queryOrNotFound(context, BrowseService.method.listEpisodes, show),
      // Only for Resume, so a failure leaves the page without it rather than failing it.
      queryClient
        .query({
          ...createQueryOptions(BrowseService.method.listContinueWatching, {}, { transport }),
          staleTime: "static",
        })
        .catch(() => null),
    ]);
    return { ...show, asked: deps.season };
  },
  pendingComponent: TitlePending,
  notFoundComponent: NotFound,
  component: Show,
});

function Show() {
  const { libraryId, showId, asked } = Route.useLoaderData();
  return <ShowPage libraryId={libraryId} showId={showId} asked={asked} />;
}
