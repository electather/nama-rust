import { createConnectQueryKey } from "@connectrpc/connect-query";
import type { InfiniteData, QueryClient } from "@tanstack/react-query";

import type { Event, SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import type { ListUnmatchedItemsResponse } from "@/gen/nama/v1/match_pb";
import { adminStopQuery } from "@/shared/hooks/admin-stop-query";
import { browseKey } from "@/shared/hooks/browse";
import { playsKey } from "@/shared/hooks/plays-key";
import { requestsKey } from "@/shared/hooks/requests-key";
import { SCANS, scanQuery } from "@/shared/hooks/scan-query";
import { searchKey } from "@/shared/hooks/search-query";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/** Updates or invalidates the queries the subscription's `response` is about. */
export function applyEvent({ message }: SubscribeResponse, queryClient: QueryClient) {
  if (message.case === "event") {
    recordScan(message.value, queryClient);
    relistUnmatched(message.value, queryClient);
    dropDecided(message.value, queryClient);
    refreshBrowsing(message.value, queryClient);
    refreshLibrary(message.value, queryClient);
    refreshRequests(message.value, queryClient);
    refreshPlays(message.value, queryClient);
    recordAdminStop(message.value, queryClient);
  } else if (message.case === "lagged") {
    forgetMissed(queryClient);
  }
}

/**
 * Refetches everything after Events were missed. Scans can't be refetched, so what's known of them
 * is forgotten rather than left stuck mid-scan.
 */
export function forgetMissed(queryClient: QueryClient) {
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

/**
 * Lists the Items a finished scan left Unmatched, and again once Matching finishes: why it left
 * each one Unmatched is stored only then.
 */
function relistUnmatched({ event }: Event, queryClient: QueryClient) {
  if (event.case === "scanFinished" || event.case === "matchingFinished") {
    void queryClient.invalidateQueries({ queryKey: unmatchedKey(event.value.libraryId) });
  }
}

/** Drops each Unmatched Item that is Matched or gone. */
function dropDecided({ event }: Event, queryClient: QueryClient) {
  if (event.case === "itemMatched" || event.case === "libraryItemRemoved") {
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
  queryClient.setQueriesData<InfiniteData<ListUnmatchedItemsResponse>>(
    { queryKey },
    (list) =>
      list && {
        ...list,
        pages: list.pages.map((page) => ({
          ...page,
          items: page.items.filter((item) => item.itemId !== itemId),
        })),
      },
  );
  void queryClient.invalidateQueries({ queryKey });
}

/**
 * Refetches the Movies, Shows and Episodes on screen when an Item is Matched or removed, or a scan
 * finishes: a scan records which folders are read-only, which can change with no Item Matched or
 * removed.
 */
function refreshBrowsing({ event }: Event, queryClient: QueryClient) {
  if (
    event.case === "itemMatched" ||
    event.case === "libraryItemRemoved" ||
    event.case === "scanFinished"
  ) {
    // ponytail: every browse query, not only those of the Event's Library. Only the ones on screen
    // refetch, and a household Server has a handful of Libraries.
    void queryClient.invalidateQueries({ queryKey: browseKey });
  }
}

/** Refetches a Library when its scan finishes, which records the folders that look empty. */
function refreshLibrary({ event }: Event, queryClient: QueryClient) {
  if (event.case !== "scanFinished") {
    return;
  }
  const input = { id: event.value.libraryId };
  const method = LibraryService.method.getLibrary;
  const queryKey = createConnectQueryKey({ schema: method, input, cardinality: "finite" });
  void queryClient.invalidateQueries({ queryKey });
}

const requestEvents = new Set<Event["event"]["case"]>([
  "requestCreated",
  "requesterAdded",
  "requestStateChanged",
  "requestCancelled",
  "requesterRemoved",
  "requestWithdrawn",
  "requestProgress",
  "followedShowCreated",
  "followedShowRequesterAdded",
  "followedShowStateChanged",
  "followedShowCancelled",
  "followedShowRequesterRemoved",
  "followedShowWithdrawn",
]);

/**
 * Refetches the Requests when a Request or Followed Show opens, gains or loses a Requester, changes
 * state or goes, or a Request's downloads move.
 */
function refreshRequests({ event }: Event, queryClient: QueryClient) {
  if (requestEvents.has(event.case)) {
    void queryClient.invalidateQueries({ queryKey: requestsKey });
    void queryClient.invalidateQueries({ queryKey: searchKey });
  }
}

/**
 * Refetches the plays when one starts, changes or ends. The events don't carry a play's title,
 * progress or where it Transcodes, so the list is read again rather than patched by plan id.
 */
function refreshPlays({ event }: Event, queryClient: QueryClient) {
  if (
    event.case === "playbackStarted" ||
    event.case === "playbackChanged" ||
    event.case === "playbackEnded"
  ) {
    void queryClient.invalidateQueries({ queryKey: playsKey });
  }
}

/** Keeps an Admin's stop of a plan, so its player closes, wherever it plays. */
function recordAdminStop({ event }: Event, queryClient: QueryClient) {
  if (event.case === "playbackEnded" && event.value.playback && event.value.adminStop) {
    const { queryKey } = adminStopQuery(event.value.playback.planId);
    queryClient.setQueryData(queryKey, event.value.adminStop);
  }
}
