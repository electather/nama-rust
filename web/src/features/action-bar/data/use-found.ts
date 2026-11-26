import { type UseQueryResult, useQueries, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";

import {
  type ServerToSearch,
  useServersToSearch,
} from "@/features/action-bar/data/use-servers-to-search";
import { foundIn } from "@/features/action-bar/lib/titles";
import type { TitleKind } from "@/gen/nama/v1/match_pb";
import type { SearchResponse } from "@/gen/nama/v1/search_pb";
import { searchQuery } from "@/shared/hooks/search-query";

/**
 * What a search for `words` of `kind` finds, in the signed-in User's Libraries and to request, and
 * nothing for no words. At the Hub, it's what each of the Account's Servers finds, as one list
 * (ADR 0105). The last answer stays while the next one loads, and a new search aborts the last.
 */
export function useFound(words: string, kind: TitleKind) {
  const queryClient = useQueryClient();
  const { servers, reaching } = useServersToSearch();
  // The last words any Server answered, whose answers stay listed while the next ones load.
  const [kept, setKept] = useState("");
  const asked = words !== "";
  // Not suspended: the list keeps its last results, and offers Retry as one of its rows, while it searches.
  const search = useQueries({
    queries: servers.map(({ transport }) => ({
      ...searchQuery(words, kind, transport),
      enabled: asked,
      // Not `keepPreviousData`: `useQueries` watches new words with a new observer, which has no
      // last answer to keep.
      placeholderData: () => queryClient.getQueryData(searchQuery(kept, kind, transport).queryKey),
    })),
    combine: (answers) => {
      const searching = asked && isSearching(answers, reaching);
      const found = asked ? foundIn(answersToList(servers, answers, searching)) : undefined;
      return {
        found,
        note: found?.note,
        searching,
        failed: allFailed(answers),
        answered: answers.some(
          ({ data, isPlaceholderData }) => data !== undefined && !isPlaceholderData,
        ),
        retry: () => {
          for (const answer of answers.filter(({ isError }) => isError)) {
            void answer.refetch();
          }
        },
      };
    },
  });
  if (search.answered && kept !== words) {
    setKept(words);
  }
  return search;
}

/** One Server's answer to a search, as it stands. */
type Answer = UseQueryResult<SearchResponse[]>;

/**
 * The answers of `servers` to list. The last words' answers stay only while every Server is still
 * `searching` and none has answered the words asked; after that they'd list titles for other words.
 */
function answersToList(
  servers: readonly ServerToSearch[],
  answers: readonly Answer[],
  searching: boolean,
) {
  const answered = answers.flatMap(({ data, isPlaceholderData }, at) =>
    data === undefined ? [] : [{ server: servers[at]?.server, responses: data, isPlaceholderData }],
  );
  const fresh = answered.filter(({ isPlaceholderData }) => !isPlaceholderData);
  return searching && fresh.length === 0 ? answered : fresh;
}

/**
 * Whether a search is still on its way: until the first Server is done, so a slow one holds up
 * none of the rest (ADR 0105), or while there's none to ask yet but one is being reached.
 */
function isSearching(answers: readonly Answer[], reaching: boolean) {
  return answers.length === 0 ? reaching : answers.every(({ isFetching }) => isFetching);
}

/** Whether every Server asked failed, with one at least. */
function allFailed(answers: readonly Answer[]) {
  return answers.length > 0 && answers.every(({ isError }) => isError);
}
