import { createQueryOptions } from "@connectrpc/connect-query";
import {
  type QueryClient,
  type UseQueryResult,
  useQueries,
  useQueryClient,
} from "@tanstack/react-query";
import { useState } from "react";

import { useReachedServers } from "@/features/home/data/use-reached-servers";
import { entryCard } from "@/features/home/lib/continue-watching-cards";
import type { ReachedServer } from "@/features/home/lib/hub-rows";
import { cardOn, mergeContinueWatching } from "@/features/home/lib/hub-titles";
import { BrowseService, type ListContinueWatchingResponse } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

const pageSize = 20;

/**
 * Continue Watching at the Hub: each Server's that has answered so far, as one, each title once,
 * from the Server it was last watched on; the Servers that `failed`; whether it's still `waiting`
 * for a Server's first page, or `complete`; and its `next` pages, each Server's, as it scrolls.
 */
export function useHubContinueWatching() {
  const reached = useReachedServers();
  const queryClient = useQueryClient();
  // How many pages past its first each Server's shelf has asked for, by the Server's id.
  const [asked, setAsked] = useState<Record<string, number>>({});
  const pages = reached.flatMap((on) => pageQueries(queryClient, on, asked[on.server.id] ?? 0));
  // Not suspended, as `useReachedServers` isn't.
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
  let query = pageQuery(on, "");
  const pages = [{ on, query }];
  for (let left = after; left > 0; left -= 1) {
    const token = tokenAfter(queryClient.getQueryData(query.queryKey));
    if (token === undefined) {
      break;
    }
    query = pageQuery(on, token);
    pages.push({ on, query });
  }
  return pages;
}

function pageQuery(on: ReachedServer, token: string) {
  return {
    ...createQueryOptions(
      BrowseService.method.listContinueWatching,
      { page: { size: pageSize, token } },
      { transport: on.transport },
    ),
    staleTime: loaderStaleTime,
  };
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
