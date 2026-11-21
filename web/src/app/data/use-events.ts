import { createClient, type Transport } from "@connectrpc/connect";
import { useTransport } from "@connectrpc/connect-query";
import { type QueryClient, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { applyEvent, forgetMissed } from "@/app/data/apply-event";
import { EventService, EventType } from "@/gen/nama/v1/event_pb";
import { playsKey } from "@/shared/hooks/plays-key";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/**
 * The Events the app acts on. Asking for these alone keeps down the traffic of the rest, such as
 * each file a scan adds.
 */
const types = [
  EventType.SCAN_STARTED,
  EventType.SCAN_PROGRESS,
  EventType.SCAN_FINISHED,
  EventType.MATCHING_FINISHED,
  EventType.LIBRARY_ITEM_REMOVED,
  EventType.ITEM_MATCHED,
  EventType.REQUEST_CREATED,
  EventType.REQUESTER_ADDED,
  EventType.REQUEST_STATE_CHANGED,
  EventType.REQUEST_CANCELLED,
  EventType.REQUESTER_REMOVED,
  EventType.REQUEST_WITHDRAWN,
  EventType.REQUEST_PROGRESS,
  EventType.FOLLOWED_SHOW_CREATED,
  EventType.FOLLOWED_SHOW_REQUESTER_ADDED,
  EventType.FOLLOWED_SHOW_STATE_CHANGED,
  EventType.FOLLOWED_SHOW_CANCELLED,
  EventType.FOLLOWED_SHOW_REQUESTER_REMOVED,
  EventType.FOLLOWED_SHOW_WITHDRAWN,
  EventType.PLAYBACK_STARTED,
  EventType.PLAYBACK_CHANGED,
  EventType.PLAYBACK_ENDED,
];

// How long to wait before subscribing again after the stream ends or fails.
const RESUBSCRIBE_MS = 5000;

/**
 * Follows the Server's Events while mounted, the app's one subscription: each Event updates or
 * invalidates the queries it's about.
 */
export function useEvents() {
  const transport = useTransport();
  const queryClient = useQueryClient();
  useEffect(() => {
    const hangUp = new AbortController();
    void follow({ transport, queryClient, signal: hangUp.signal });
    return () => {
      hangUp.abort();
    };
  }, [transport, queryClient]);
}

interface Following {
  transport: Transport;
  queryClient: QueryClient;
  signal: AbortSignal;
}

/**
 * Subscribes, and again whenever the stream ends, until `signal` aborts. Once a stream opens, what
 * the Events before it would have changed is read again: on the first, the Unmatched Items and plays
 * loaded while subscribing; on a later one, everything, as after `Lagged`. A Server that stays down costs
 * one failed call per retry and nothing more.
 */
async function follow(following: Following, resubscribing = false) {
  const { transport, queryClient, signal } = following;
  const catchUp = () => {
    if (resubscribing) {
      forgetMissed(queryClient);
    } else {
      void queryClient.invalidateQueries({ queryKey: unmatchedKey() });
      void queryClient.invalidateQueries({ queryKey: playsKey });
    }
  };
  try {
    const client = createClient(EventService, transport);
    const responses = client.subscribe({ types }, { signal, onHeader: catchUp });
    for await (const response of responses) {
      applyEvent(response, queryClient);
    }
  } catch {
    // The Server went away or hung up, or the session ended and the routes' access checks will
    // notice. Subscribing again below is all there is to do.
  }
  setTimeout(() => {
    if (!signal.aborted) {
      void follow(following, true);
    }
  }, RESUBSCRIBE_MS);
}
