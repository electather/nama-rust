import { type UseQueryResult, keepPreviousData, useQueries } from "@tanstack/react-query";

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
  const { servers, reaching } = useServersToSearch();
  const asked = words !== "";
  // Not suspended: the list keeps its last results, and offers Retry as one of its rows, while it searches.
  return useQueries({
    queries: servers.map(({ transport }) => ({
      ...searchQuery(words, kind, transport),
      enabled: asked,
      placeholderData: keepPreviousData,
    })),
    combine: (answers) => {
      const found = asked ? foundIn(answersToList(servers, answers)) : undefined;
      return {
        found,
        note: found?.note,
        searching: asked && isSearching(answers, reaching),
        failed: allFailed(answers),
        retry: () => {
          for (const answer of answers.filter(({ isError }) => isError)) {
            void answer.refetch();
          }
        },
      };
    },
  });
}

/** One Server's answer to a search, as it stands. */
type Answer = UseQueryResult<SearchResponse[]>;

/**
 * The answers of `servers` to list: once one answers the words asked, the others' answers to the
 * last words are dropped.
 */
function answersToList(servers: readonly ServerToSearch[], answers: readonly Answer[]) {
  const answered = answers.flatMap(({ data, isPlaceholderData }, at) =>
    data === undefined ? [] : [{ server: servers[at]?.server, responses: data, isPlaceholderData }],
  );
  const fresh = answered.filter(({ isPlaceholderData }) => !isPlaceholderData);
  return fresh.length > 0 ? fresh : answered;
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
