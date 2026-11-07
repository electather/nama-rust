import { createClient, type Transport } from "@connectrpc/connect";
import { createConnectQueryKey, useTransport } from "@connectrpc/connect-query";
import { type QueryClient, queryOptions, skipToken, useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import {
  type Event,
  EventService,
  EventType,
  type ScanFinished,
  type SubscribeResponse,
  type ScanProgress,
  type ScanStarted,
} from "@/gen/nama/v1/event_pb";
import { type ListUnmatchedItemsResponse, MatchService } from "@/gen/nama/v1/match_pb";
import { RequestService } from "@/gen/nama/v1/request_pb";

import { browseKey } from "./browse";

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

/**
 * The Events the app acts on. Asking for these alone keeps the rest, such as each file a scan
 * adds with its path on the Server, off every signed-in browser.
 */
const types = [
  EventType.SCAN_STARTED,
  EventType.SCAN_PROGRESS,
  EventType.SCAN_FINISHED,
  EventType.LIBRARY_ITEM_REMOVED,
  EventType.ITEM_MATCHED,
  EventType.REQUEST_CREATED,
  EventType.REQUESTER_ADDED,
  EventType.REQUEST_STATE_CHANGED,
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
    void follow(transport, queryClient, hangUp.signal);
    return () => {
      hangUp.abort();
    };
  }, [transport, queryClient]);
}

/**
 * Subscribes, and again whenever the stream ends, until `signal` aborts. Once a stream opens, what
 * the Events before it would have changed is read again: on the first, the Unmatched Items loaded
 * while subscribing; on a later one, everything, as after `Lagged`. A Server that stays down costs
 * one failed call per retry and nothing more.
 */
async function follow(
  transport: Transport,
  queryClient: QueryClient,
  signal: AbortSignal,
  resubscribing = false,
) {
  const catchUp = () => {
    if (resubscribing) forgetMissed(queryClient);
    else void queryClient.invalidateQueries({ queryKey: unmatchedKey() });
  };
  try {
    const client = createClient(EventService, transport);
    const responses = client.subscribe({ types }, { signal, onHeader: catchUp });
    for await (const response of responses) apply(response, queryClient);
  } catch {
    // The Server went away or hung up, or the session ended and the routes' access checks will
    // notice. Subscribing again below is all there is to do.
  }
  setTimeout(() => {
    if (!signal.aborted) void follow(transport, queryClient, signal, true);
  }, RESUBSCRIBE_MS);
}

function apply({ message }: SubscribeResponse, queryClient: QueryClient) {
  if (message.case === "event") {
    recordScan(message.value, queryClient);
    refreshUnmatched(message.value, queryClient);
    refreshBrowsing(message.value, queryClient);
    refreshRequests(message.value, queryClient);
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

/** Refetches the Movies, Shows and Episodes on screen when an Item is Matched or removed. */
function refreshBrowsing({ event }: Event, queryClient: QueryClient) {
  if (event.case === "itemMatched" || event.case === "libraryItemRemoved") {
    // ponytail: every browse query, not only those of the Event's Library. Only the ones on screen
    // refetch, and a household Server has a handful of Libraries.
    void queryClient.invalidateQueries({ queryKey: browseKey });
  }
}

/** Refetches the Requests when one opens, gains a Requester or changes state. */
function refreshRequests({ event }: Event, queryClient: QueryClient) {
  if (
    event.case === "requestCreated" ||
    event.case === "requesterAdded" ||
    event.case === "requestStateChanged"
  ) {
    void queryClient.invalidateQueries({ queryKey: requestsKey });
  }
}

/** Every Request query, including the Movies found to Request: what to refresh when one changes. */
export const requestsKey = createConnectQueryKey({
  schema: RequestService,
  cardinality: undefined,
});
