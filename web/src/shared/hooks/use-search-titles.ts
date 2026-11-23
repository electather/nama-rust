import { useQuery } from "@connectrpc/connect-query";

import { MatchService, type TitleKind } from "@/gen/nama/v1/match_pb";

/**
 * The titles the Library `libraryId`'s title sources found for `search`, as a query that says while
 * it searches and why it failed.
 */
export function useSearchTitles(search: {
  kind: TitleKind;
  title: string;
  year: number | undefined;
  libraryId: bigint;
}) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the results say "Searching…" and show why a search failed beside the form
  return useQuery(MatchService.method.searchTitles, search);
}
