import { createQueryOptions } from "@connectrpc/connect-query";
import { type UseQueryResult, useQueries } from "@tanstack/react-query";
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
  // The page tokens asked of each Server, by its id, after its first page's.
  const [asked, setAsked] = useState<Record<string, string[]>>({});
  const pages = reached.flatMap((on) =>
    ["", ...(asked[on.server.id] ?? [])].map((token) => ({ on, token })),
  );
  // Not suspended, as `useReachedServers` isn't.
  const answers = useQueries({
    queries: pages.map(({ on, token }) => ({
      ...createQueryOptions(
        BrowseService.method.listContinueWatching,
        { page: { size: pageSize, token } },
        { transport: on.transport },
      ),
      staleTime: loaderStaleTime,
    })),
  });
  const servers = reached.map((on) =>
    serverSoFar(
      on,
      answers.filter((_, at) => pages[at]?.on === on),
    ),
  );
  const load = () => {
    setAsked((was) => {
      const due = servers.flatMap(({ on, next }) =>
        next === undefined || was[on.server.id]?.includes(next) === true ? [] : [{ on, next }],
      );
      const now = { ...was };
      for (const { on, next } of due) {
        now[on.server.id] = [...(was[on.server.id] ?? []), next];
      }
      // The same state when there's nothing new to ask, so React doesn't render again.
      return due.length === 0 ? was : now;
    });
  };
  const waiting = servers.some((server) => server.waiting);
  const more = servers.some(({ next }) => next !== undefined);
  return {
    cards: mergeContinueWatching(servers),
    failed: servers.flatMap(({ on, failed }) => (failed ? [on.server] : [])),
    waiting,
    complete: !waiting && !more,
    next: { has: more, load },
  };
}

type Page = UseQueryResult<ListContinueWatchingResponse>;

/** One Server's Continue Watching from its `pages` so far, in order. */
function serverSoFar(on: ReachedServer, pages: Page[]) {
  const failed = pages.some(({ isError }) => isError);
  const last = pages.at(-1);
  const next = failed ? undefined : tokenAfter(last);
  return {
    on,
    cards: pages
      .flatMap(({ data }) => data?.entries ?? [])
      .flatMap((entry) => entryCard(entry))
      .map((card) => cardOn(on, card)),
    failed,
    waiting: isPending(pages[0]),
    more: next !== undefined || isPending(last),
    next,
  };
}

/** The token of the page after `page`, once it has answered with one. */
function tokenAfter(page: Page | undefined) {
  const token = page?.data?.page?.nextToken ?? "";
  return token === "" ? undefined : token;
}

function isPending(page: Page | undefined) {
  return page?.isPending === true;
}
