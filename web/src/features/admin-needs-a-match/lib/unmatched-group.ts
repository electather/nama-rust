import type { Library } from "@/gen/nama/v1/library_pb";
import type { ListUnmatchedItemsResponse, UnmatchedItem } from "@/gen/nama/v1/match_pb";

/** One Library's Unmatched Items as far as they've loaded, and how to load the next page. */
export interface UnmatchedGroup {
  library: Library;
  items: UnmatchedItem[];
  /** The files whose names say nothing to parse: no Item to Match, only to rename. */
  unparsedPaths: string[];
  /** How many files the Library has that need a Match, the unloaded pages' included. */
  count: number;
  hasNextPage: boolean;
  loadedPages: number;
  loadMore: (options: { cancelRefetch: boolean }) => Promise<unknown>;
}

/** The group of `library` that `paged`, its pages so far, make. */
export function unmatchedGroup(
  library: Library,
  paged: {
    pages: ListUnmatchedItemsResponse[];
    hasNextPage: boolean;
    totalSize: number | undefined;
    fetchNextPage: UnmatchedGroup["loadMore"];
  },
): UnmatchedGroup {
  const { pages, hasNextPage, totalSize, fetchNextPage } = paged;
  const items = pages.flatMap((page) => page.items);
  // ponytail: only the first page carries them, and `usePaged` lets it go after 10 pages; keep
  // them as it keeps `totalSize` if a Library that long turns up.
  const unparsedPaths = pages.flatMap((page) => page.unparsedPaths);
  // Past the pages loaded only the Server's own count says how many there are.
  const listed = hasNextPage ? Math.max(totalSize ?? 0, items.length) : items.length;
  return {
    library,
    items,
    unparsedPaths,
    count: listed + unparsedPaths.length,
    hasNextPage,
    loadedPages: pages.length,
    loadMore: fetchNextPage,
  };
}

/** How many files in all `groups` need a Match. */
export function filesNeedingAMatch(groups: UnmatchedGroup[]) {
  let total = 0;
  for (const { count } of groups) {
    total += count;
  }
  return total;
}
