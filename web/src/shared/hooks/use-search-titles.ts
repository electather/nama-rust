import { useQuery } from "@connectrpc/connect-query";
import { keepPreviousData } from "@tanstack/react-query";

import { MatchService, type TitleKind } from "@/gen/nama/v1/match_pb";

/**
 * The titles the Library `libraryId`'s title sources found for `search`, as a query that says while
 * it searches and why it failed. A blank title asks nothing. While a new search runs the last one's
 * titles stay, so typing never blanks the list.
 */
export function useSearchTitles(search: {
  kind: TitleKind;
  title: string;
  year: number | undefined;
  libraryId: bigint;
}) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the results say "Searching…" and show why a search failed beside the form
  return useQuery(MatchService.method.searchTitles, search, {
    enabled: search.title !== "",
    placeholderData: keepPreviousData,
  });
}
