import {
  hasTitleFilter,
  isUnmatched,
  type TitleListing,
} from "@/features/library/lib/title-listing";
import type { Paged } from "@/features/library/lib/titles-props";
import type { UnmatchedItemSummary } from "@/gen/nama/v1/browse_pb";

/** The pages of a Library's Unmatched Items, as the grid reads them. */
export type UnmatchedPages = Paged<{ items: UnmatchedItemSummary[] }>;

/**
 * What a Library's grid lists once the Unmatched Items join its titles: the `paged` it loads and
 * counts by, and the `items` that follow its titles as tiles. `only` is for when the grid holds
 * nothing else.
 */
export interface Joined {
  paged: Paged<unknown>;
  items: UnmatchedItemSummary[];
  only: boolean;
}

/** What of the titles' `list`, such as their posters or quick-scroll rail, the grid keeps: none with Not matched on. */
export function keptTitles<T>({ only }: Joined, list: T[]) {
  return only ? [] : list;
}

/**
 * The Library's titles and its Unmatched Items as one list for its grid. The tiles come after the
 * titles' last page, so they never sit among the titles still to load, and not while a title
 * filter is on, since they match none. With Not matched on, they're all the grid holds. Paging goes
 * through the titles first, then the Unmatched Items.
 */
export function withUnmatched(
  titles: Paged<unknown>,
  unmatched: UnmatchedPages,
  listing: TitleListing,
): Joined {
  const items = unmatched.data.pages.flatMap((page) => page.items);
  if (isUnmatched(listing)) {
    return { paged: unmatched, items, only: true };
  }
  if (hasTitleFilter(listing)) {
    return { paged: titles, items: [], only: false };
  }
  return {
    paged: {
      data: { pages: [...titles.data.pages, ...unmatched.data.pages] },
      hasNextPage: titles.hasNextPage || unmatched.hasNextPage,
      fetchNextPage: titles.hasNextPage ? titles.fetchNextPage : unmatched.fetchNextPage,
      totalSize: (titles.totalSize ?? 0) + (unmatched.totalSize ?? 0),
    },
    items: titles.hasNextPage ? [] : items,
    only: false,
  };
}
