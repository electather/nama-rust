import { useTransport } from "@connectrpc/connect-query";
import { useQuery } from "@tanstack/react-query";

import type { TitleKind } from "@/gen/nama/v1/match_pb";
import { searchQuery } from "@/shared/hooks/search-query";
import { hitsIn, searchNote } from "@/shared/lib/search-hits";

/** The Movies and Shows of `kind` a search for `words` finds, each with its status, best first. */
export function useTitlesFound(words: string, kind: TitleKind) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- a search shows "Searching…" and its own error under the page's form, which stays up
  const query = useQuery(searchQuery(words, kind, useTransport()));
  const responses = query.data ?? [];
  const hits = hitsIn(responses);
  return {
    movies: hits.flatMap(({ title }) => (title.case === "movie" ? [title.value] : [])),
    shows: hits.flatMap(({ title, status }) =>
      title.case === "show" ? [{ show: title.value, status }] : [],
    ),
    note: searchNote(responses),
    searching: query.isFetching,
    failed: query.isError,
  };
}
