import { createClient, type Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useTransport } from "@connectrpc/connect-query";
import { type QueryClient, queryOptions, skipToken, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import {
  type Event,
  EventService,
  type ScanFinished,
  type SubscribeResponse,
  type ScanProgress,
  type ScanStarted,
} from "@/gen/nama/v1/event_pb";
import { type ListUnmatchedItemsResponse, MatchService } from "@/gen/nama/v1/match_pb";

/** The latest scan Event about a Library, if one arrived since the page was opened. */
export type ScanState = ScanStarted | ScanProgress | ScanFinished;

const SCANS = "scan";

/** Where the latest scan Event about the Library `libraryId` is kept. Only Events fill it. */
export function scanQuery(libraryId: bigint) {
  return queryOptions<ScanState>({ queryKey: [SCANS, String(libraryId)], queryFn: skipToken });
}

/** The query key of the Library `libraryId`'s Unmatched Items, or every Library's without one. */
export function unmatchedKey(libraryId?: bigint) {
  return createConnectQueryKey({
    schema: MatchService.method.listUnmatchedItems,
    ...(libraryId !== undefined && { input: { libraryId } }),
    cardinality: "finite",
  });
}

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
    void follow(transport, queryClient, hangUp.signal);
    return () => {
      hangUp.abort();
    };
  }, [transport, queryClient]);
}

async function follow(transport: Transport, queryClient: QueryClient, signal: AbortSignal) {
  try {
    const responses = createClient(EventService, transport).subscribe({}, { signal });
    for await (const response of responses) apply(response, queryClient);
  } catch {
    // The Server went away or hung up. Subscribing again below is all there is to do.
  }
  setTimeout(() => {
    if (signal.aborted) return;
    forgetMissed(queryClient);
    void follow(transport, queryClient, signal);
  }, RESUBSCRIBE_MS);
}

function apply({ message }: SubscribeResponse, queryClient: QueryClient) {
  if (message.case === "event") {
    recordScan(message.value, queryClient);
    refreshUnmatched(message.value, queryClient);
  } else if (message.case === "lagged") {
    forgetMissed(queryClient);
  }
}

/**
 * Refetches everything after Events were missed. Scans can't be refetched, so what's known of them
 * is forgotten rather than left stuck mid-scan.
 */
function forgetMissed(queryClient: QueryClient) {
  void queryClient.resetQueries({ queryKey: [SCANS] });
  void queryClient.invalidateQueries();
}

function recordScan({ event }: Event, queryClient: QueryClient) {
  if (
    event.case === "scanStarted" ||
    event.case === "scanProgress" ||
    event.case === "scanFinished"
  ) {
    queryClient.setQueryData(scanQuery(event.value.libraryId).queryKey, event.value);
  }
}

/** Lists the Items a finished scan left Unmatched, and drops each one Matched or gone. */
function refreshUnmatched({ event }: Event, queryClient: QueryClient) {
  if (event.case === "scanFinished") {
    void queryClient.invalidateQueries({ queryKey: unmatchedKey(event.value.libraryId) });
  } else if (event.case === "itemMatched" || event.case === "libraryItemRemoved") {
    dropUnmatched(queryClient, event.value.libraryId, event.value.itemId);
  }
}

/**
 * Takes the Item `itemId` off its Library's Unmatched Items at once, then refetches them. That
 * also restarts a refetch begun before this Event, whose answer would bring the Item back.
 */
// ponytail: one refetch per Event while the list is on screen; debounce if a big scan's burst of
// Matches turns out to load the Server.
function dropUnmatched(queryClient: QueryClient, libraryId: bigint, itemId: bigint) {
  const queryKey = unmatchedKey(libraryId);
  queryClient.setQueriesData<ListUnmatchedItemsResponse>(
    { queryKey },
    (list) => list && { ...list, items: list.items.filter((item) => item.itemId !== itemId) },
  );
  void queryClient.invalidateQueries({ queryKey });
}
