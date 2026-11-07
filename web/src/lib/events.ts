import { type Transport, createClient } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { EventService, EventType } from "@/gen/nama/v1/event_pb";

import { browseKey } from "./browse";

/** The Events that change what browsing shows. */
const types = [EventType.ITEM_MATCHED, EventType.LIBRARY_ITEM_REMOVED];

/** How long to wait before subscribing again after the stream ends, such as on a Server restart. */
const RESUBSCRIBE_MS = 5000;

/**
 * Keeps what's on screen current while you browse: an Item Matched or removed in a Library
 * refetches the Movies, Shows and Episodes shown, for as long as the calling component is mounted.
 */
export function useBrowseRefreshes() {
  const transport = useTransport();
  const queryClient = useQueryClient();
  useEffect(() => {
    const unmounted = new AbortController();
    // ponytail: every browse query, not only those of the Event's Library. Only the ones on screen
    // refetch, and a household Server has a handful of Libraries.
    const refresh = () => queryClient.invalidateQueries({ queryKey: browseKey });
    void refreshOnEvents(transport, refresh, unmounted.signal);
    return () => {
      unmounted.abort();
    };
  }, [transport, queryClient]);
}

async function refreshOnEvents(
  transport: Transport,
  refresh: QueryClient["invalidateQueries"],
  signal: AbortSignal,
) {
  try {
    // An Event and a `Lagged` (Events dropped) alike mean something changed.
    for await (const _ of createClient(EventService, transport).subscribe({ types }, { signal })) {
      void refresh();
    }
  } catch {
    // The Server went away, or the session ended and the routes' access checks will notice.
  }
  setTimeout(() => {
    if (signal.aborted) return;
    // Whatever happened while unsubscribed went unseen.
    void refresh();
    void refreshOnEvents(transport, refresh, signal);
  }, RESUBSCRIBE_MS);
}
