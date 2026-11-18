import type { ListUnmatchedItemsResponse } from "@/gen/nama/v1/match_pb";

/**
 * The Items and unparsed paths `pages` list, whether their Library needs a Match, and whether it
 * or the Libraries before it (`anyBefore`) do. Pages yet to load (`hasNextPage`) count too, as an
 * Event can take every loaded Item off before they load.
 */
export function loaded(
  pages: ListUnmatchedItemsResponse[],
  hasNextPage: boolean,
  anyBefore: boolean,
) {
  const items = pages.flatMap((page) => page.items);
  // ponytail: only the first page carries them, and `usePaged` lets it go after 10 pages; keep
  // them as it keeps `totalSize` if a Library that long turns up.
  const unparsedPaths = pages.flatMap((page) => page.unparsedPaths);
  const needsAMatch = items.length + unparsedPaths.length > 0 || hasNextPage;
  return { items, unparsedPaths, needsAMatch, anyYet: anyBefore || needsAMatch };
}
