import { isMessage } from "@bufbuild/protobuf";
import type { Transport } from "@connectrpc/connect";
import { callUnaryMethod, useTransport } from "@connectrpc/connect-query";
import { type Query, type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";

import {
  PlanPlaybackResponseSchema,
  type PlaybackPlan,
  PlaybackService,
} from "@/gen/nama/v1/playback_pb";

/** What every plan's query key starts with. */
export const planKey = ["planPlayback"] as const;

/**
 * How often an open plan asks for its URL. The Server stops a plan a minute after it was last
 * asked for, and a paused player, its buffer full, asks for nothing.
 */
const KEEP_ALIVE_MS = 30_000;

/**
 * A Worker's script that, sent a number of milliseconds, posts an empty message that often (why a
 * Worker: `features/player/AGENTS.md`). Inline: a file of its own would import nothing, which
 * oxlint's `import/unambiguous` rejects with no line to suppress it on. A page Chrome freezes stops its
 * Workers too, and plans again when it's back (#2084).
 */
const TICKER_URL = URL.createObjectURL(
  new Blob(["onmessage = ({ data }) => setInterval(() => postMessage(null), data);"], {
    type: "text/javascript",
  }),
);

/**
 * The plans one Query client's players ended, kept outside React since a stop fires after its
 * player is gone.
 */
interface Ended {
  /** The plans a player let go of, by id, each to be stopped once the task that let go ends. */
  letGo: Map<bigint, ReturnType<typeof setTimeout>>;
  /** The plans stopped while their player was still open, by id, which it then needn't stop. */
  stoppedEarly: Set<bigint>;
}

/** What a plan's lifetime needs from the app it plays in. */
interface Plans extends Ended {
  queryClient: QueryClient;
  transport: Transport;
}

type Playing = Pick<PlaybackPlan, "id" | "url">;

// Per Query client, not one set for the page, so each test's app starts with no plans ended.
const endedBy = new WeakMap<QueryClient, Ended>();

/**
 * The lifetime of the Query client's Playback Plans, each stopped exactly once and forgotten once
 * nothing shows it. `stopReplaced` tells the Server a plan stopped while its player is still open,
 * as before asking for the plan that replaces it, so a Transcode it runs no longer counts toward
 * the Server's limit, and forgets it. `keepPlaying` is an effect that keeps an open plan alive
 * until the page goes or its cleanup, either of which stops it. The rule it keeps is in
 * `features/player/AGENTS.md`.
 */
export function usePlanLifetime() {
  const queryClient = useQueryClient();
  const transport = useTransport();
  return useMemo(() => {
    const plans = { queryClient, transport, ...endedIn(queryClient) };
    return {
      stopReplaced: async (planId: bigint) => {
        // Now, before the plan that replaces it is cached: a restarted Server may give it this id.
        forget(queryClient, planId);
        await stopEarly(plans, planId);
      },
      keepPlaying: (plan: Playing) => keepPlaying(plans, plan),
    };
  }, [queryClient, transport]);
}

function endedIn(queryClient: QueryClient) {
  const ended = endedBy.get(queryClient) ?? { letGo: new Map(), stoppedEarly: new Set() };
  endedBy.set(queryClient, ended);
  return ended;
}

async function stopEarly(plans: Plans, planId: bigint) {
  plans.stoppedEarly.add(planId);
  await tellStopped(plans.transport, planId);
}

/**
 * Asks for the plan `planId`'s `url` every `KEEP_ALIVE_MS`. The cleanup it returns stops the plan
 * and forgets it, so opening the player again asks for a new plan.
 */
function keepPlaying(plans: Plans, { id: planId, url }: Playing) {
  // React may clean up an effect and set it up again for the same plan, as `StrictMode` does on
  // mounting: a stop sent at once would end the plan the `<video>` goes on playing.
  clearTimeout(plans.letGo.get(planId));
  plans.letGo.delete(planId);
  const ticker = new Worker(TICKER_URL);
  ticker.addEventListener("message", () => {
    void ask(url);
  });
  // oxlint-disable-next-line unicorn/require-post-message-target-origin -- a Worker's, which takes no origin
  ticker.postMessage(KEEP_ALIVE_MS);
  // Closing or reloading the tab unmounts nothing, and a reloaded one no longer knows this plan,
  // whose Transcode would count toward the limit for that minute. A page the browser keeps to go
  // back to plays on when it does. ponytail: best effort, as the page may go before it's sent.
  const leaving = ({ persisted }: PageTransitionEvent) => {
    if (!persisted) {
      void stopEarly(plans, planId);
    }
  };
  globalThis.addEventListener("pagehide", leaving);
  return () => {
    ticker.terminate();
    globalThis.removeEventListener("pagehide", leaving);
    stopLater(plans, planId);
  };
}

function stopLater(plans: Plans, planId: bigint) {
  if (plans.stoppedEarly.delete(planId)) {
    return;
  }
  const stop = setTimeout(() => {
    plans.letGo.delete(planId);
    forget(plans.queryClient, planId);
    void tellStopped(plans.transport, planId);
  }, 0);
  plans.letGo.set(planId, stop);
}

/** A failed stop is left to the minute. */
async function tellStopped(transport: Transport, planId: bigint) {
  try {
    await callUnaryMethod(transport, PlaybackService.method.stopPlayback, { planId });
  } catch {
    // Left to the minute.
  }
}

/**
 * `HEAD`, so a Direct Play's whole file isn't sent. A failed ask is left to the player, which
 * notices when it next needs a segment.
 */
async function ask(url: string) {
  try {
    await fetch(url, { method: "HEAD" });
  } catch {
    // Left to the player.
  }
}

/**
 * Drops the plan `planId` from the cache once nothing shows it, so the Version's next player asks
 * for a new one, though Suspense keeps an unseen query for a second.
 */
function forget(queryClient: QueryClient, planId: bigint) {
  const queries = queryClient.getQueryCache().findAll({
    queryKey: planKey,
    predicate: ({ state: { data } }) =>
      isMessage(data, PlanPlaybackResponseSchema) && data.plan?.id === planId,
  });
  for (const query of queries) {
    removeOnceUnseen(queryClient, query);
  }
}

// An Admin's stop, or a player that gave up, still shows the plan its `<video>` left: removed then,
// the next render would ask the Server for a new plan and play it.
function removeOnceUnseen(queryClient: QueryClient, query: Query) {
  const cache = queryClient.getQueryCache();
  if (query.getObserversCount() === 0) {
    cache.remove(query);
    return;
  }
  const unsubscribe = cache.subscribe((event) => {
    if (event.query === query && query.getObserversCount() === 0) {
      unsubscribe();
      cache.remove(query);
    }
  });
}
