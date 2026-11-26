import type { Transport } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { type UseQueryResult, keepPreviousData, useQueries, useQuery } from "@tanstack/react-query";
import { useRouteContext } from "@tanstack/react-router";

import { foundIn } from "@/features/action-bar/lib/titles";
import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import type { TitleKind } from "@/gen/nama/v1/match_pb";
import type { SearchResponse } from "@/gen/nama/v1/search_pb";
import { searchQuery } from "@/shared/hooks/search-query";
import { linkedServersQuery } from "@/shared/hooks/use-linked-servers";

/**
 * What a search for `words` of `kind` finds, in the signed-in User's Libraries and to request, and
 * nothing for no words. At the Hub, it's what each of the Account's Servers finds, as one list
 * (ADR 0105). The last answer stays while the next one loads, and a new search aborts the last.
 */
export function useFound(words: string, kind: TitleKind) {
  const servers = useServersToSearch();
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
        // Until the first Server is done, so a slow one holds up none of the rest (ADR 0105).
        searching: everyOne(answers, "isFetching"),
        failed: everyOne(answers, "isError"),
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

/** Whether every Server's answer is `state`, with one at least. A search not asked is neither. */
function everyOne(answers: readonly Answer[], state: "isFetching" | "isError") {
  return answers.length > 0 && answers.every((answer) => answer[state]);
}

/** A Server a search asks, and at the Hub, its Linked Server id. */
interface ServerToSearch {
  server: string | undefined;
  transport: Transport;
}

/**
 * The Servers a search asks: this one, or at the Hub, each of the Account's Servers it has reached,
 * by its Linked Server id.
 */
function useServersToSearch(): ServerToSearch[] {
  const transport = useTransport();
  const atHub = useRouteContext({ from: "/_member", select: ({ account }) => account !== null });
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- a Server's own web app has no Hub to ask, so it mustn't suspend there
  const linked = useQuery({ ...linkedServersQuery(transport), enabled: atHub });
  const reached = useReachedServers(linked.data?.servers ?? []);
  return atHub
    ? reached.map(({ server, transport: at }) => ({ server: server.id, transport: at }))
    : [{ server: undefined, transport }];
}
