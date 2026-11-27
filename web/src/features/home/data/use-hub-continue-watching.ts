import { isMessage } from "@bufbuild/protobuf";
import {
  callUnaryMethod,
  createConnectQueryKey,
  createQueryOptions,
} from "@connectrpc/connect-query";
import {
  type QueryClient,
  type UseQueryResult,
  useQueries,
  useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";

import { entryCard } from "@/features/home/lib/continue-watching-cards";
import type { ReachedServer } from "@/features/home/lib/hub-rows";
import { cardOn, mergeContinueWatching } from "@/features/home/lib/hub-titles";
import { useReachedServers } from "@/features/servers/data/use-reached-servers";
import {
  BrowseService,
  type ListContinueWatchingResponse,
  ListContinueWatchingResponseSchema,
} from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { useLinkedServers } from "@/shared/hooks/use-linked-servers";

const pageSize = 20;

/**
 * Continue Watching at the Hub: each Server's that has answered so far, as one, each title once,
 * from the Server it was last watched on; the Servers that `failed`; whether it's still `waiting`
 * for a Server's first page, or `complete`; and its `next` pages, each Server's, as it scrolls.
 */
export function useHubContinueWatching() {
  const { reached } = useReachedServers(useLinkedServers());
  const queryClient = useQueryClient();
  // How many pages past its first each Server's shelf has asked for, by the Server's id.
  const [asked, setAsked] = useState<Record<string, number>>({});
  const pages = reached.flatMap((on) => pageQueries(queryClient, on, asked[on.server.id] ?? 0));
  // Not suspended, as the servers feature's `useReachedServers` isn't.
  const answers = useQueries({ queries: pages.map(({ query }) => query) });
  const servers = reached.map((on) =>
    serverSoFar(
      on,
      answers.filter((_, at) => pages[at]?.on === on),
    ),
  );
  const load = () => {
    setAsked((was) => {
      // A Server asked again since this render already has its next page coming.
      const due = servers.filter(
        ({ on, next }) => next && was[on.server.id] === asked[on.server.id],
      );
      const now = { ...was };
      for (const { on } of due) {
        now[on.server.id] = (asked[on.server.id] ?? 0) + 1;
      }
      return due.length === 0 ? was : now;
    });
  };
  return {
    cards: mergeContinueWatching(servers),
    failed: servers.flatMap(({ on, failed }) => (failed ? [on.server] : [])),
    waiting: servers.some(({ waiting }) => waiting),
    complete: !servers.some(({ more }) => more),
    next: { has: servers.some(({ next }) => next), load },
  };
}

/**
 * The queries for `on`'s first page and the `after` pages after it, each asked by the token the
 * page before last answered with. A page refetched with a new token moves the next one on with it,
 * so a card that slides to a later page is neither lost nor listed twice.
 */
function pageQueries(queryClient: QueryClient, on: ReachedServer, after: number) {
  let query = pageQuery(queryClient, on, { at: 0, token: "" });
  const pages = [{ on, query }];
  for (let at = 1; at <= after; at += 1) {
    const token = tokenAfter(latestAnswer(queryClient, query.queryKey) ?? query.placeholderData);
    if (token === undefined) {
      break;
    }
    query = pageQuery(queryClient, on, { at, token });
    pages.push({ on, query });
  }
  return pages;
}

/**
 * The query for `on`'s page `at`, asked by `token`. The key holds `at` too, so a page asked by a
 * new token shows what was last at `at` till it answers, rather than the shelf shrinking back.
 */
function pageQuery(
  queryClient: QueryClient,
  on: ReachedServer,
  { at, token }: { at: number; token: string },
) {
  const method = BrowseService.method.listContinueWatching;
  const input = { page: { size: pageSize, token } };
  const { queryKey, structuralSharing } = createQueryOptions(method, input, {
    transport: on.transport,
  });
  const atKey = [
    ...createConnectQueryKey({ schema: method, cardinality: "finite", transport: on.transport }),
    at,
  ];
  return {
    queryKey: [...queryKey, at],
    // connect-query's own is typed for its key, which this one extends.
    queryFn: async ({ signal }: { signal: AbortSignal }) => {
      const answer = await callUnaryMethod(on.transport, method, input, { signal });
      return answer;
    },
    structuralSharing,
    staleTime: loaderStaleTime,
    placeholderData: at === 0 ? undefined : latestAnswer(queryClient, atKey),
  };
}

/** The most recent answer cached under `queryKey`, whichever token asked for it. */
function latestAnswer(queryClient: QueryClient, queryKey: unknown[]) {
  const answered = queryClient
    .getQueryCache()
    .findAll({ queryKey, predicate: ({ state }) => state.data !== undefined });
  const last = Math.max(...answered.map(({ state }) => state.dataUpdatedAt));
  const answer = answered.find(({ state }) => state.dataUpdatedAt === last)?.state.data;
  return isMessage(answer, ListContinueWatchingResponseSchema) ? answer : undefined;
}

type Page = UseQueryResult<ListContinueWatchingResponse>;

/** One Server's Continue Watching from its `pages` so far, in order. */
function serverSoFar(on: ReachedServer, pages: Page[]) {
  const failed = pages.some(({ isError }) => isError);
  const last = pages.at(-1);
  const next = !failed && tokenAfter(last?.data) !== undefined;
  return {
    on,
    cards: pages
      .flatMap(({ data }) => data?.entries ?? [])
      .flatMap((entry) => entryCard(entry))
      .map((card) => cardOn(on, card)),
    failed,
    waiting: isPending(pages[0]),
    more: next || isPending(last),
    next,
  };
}

/** The token of the page after the one that answered `answer`, if there's one. */
function tokenAfter(answer: ListContinueWatchingResponse | undefined) {
  const token = answer?.page?.nextToken ?? "";
  return token === "" ? undefined : token;
}

function isPending(page: Page | undefined) {
  return page?.isPending === true;
}
