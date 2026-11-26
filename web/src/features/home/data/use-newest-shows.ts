import { create } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { recentlyAddedPage } from "@/features/home/lib/shelf-tiles";
import {
  BrowseService,
  ShowInLibrarySchema,
  type ShowSummary,
  ShowSort,
} from "@/gen/nama/v1/browse_pb";
import { SortDirection } from "@/gen/nama/v1/page_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import type { RowTitle } from "@/shared/lib/row-titles";

/** What `ListShows` is asked for the Shows of the Shows Library `libraryId` added to last. */
export function newestShows(libraryId: bigint) {
  return {
    libraryId,
    sort: ShowSort.ADDED,
    direction: SortDirection.DESCENDING,
    page: recentlyAddedPage,
  };
}

/** The Shows Library `libraryId`'s `shows` as a row's titles. */
export function showRowTitles(libraryId: bigint, shows: ShowSummary[]) {
  return shows.map((show): RowTitle => ({
    case: "show",
    value: create(ShowInLibrarySchema, {
      libraryId,
      id: show.id,
      title: show.title,
      posterUrl: show.posterUrl,
      posterThumbhash: show.posterThumbhash,
    }),
  }));
}

/** The Shows of the Shows Library `libraryId` with the newest added Episodes, newest first. */
export function useNewestShows(libraryId: bigint) {
  const { shows } = useSuspenseQuery(BrowseService.method.listShows, newestShows(libraryId), {
    staleTime: loaderStaleTime,
  }).data;
  return showRowTitles(libraryId, shows);
}
