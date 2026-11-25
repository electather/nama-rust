import { createConnectQueryKey } from "@connectrpc/connect-query";
import type { InfiniteData } from "@tanstack/react-query";

import { inboxKey } from "@/features/inbox/data/inbox-key";
import type { Event, SubscribeResponse } from "@/gen/nama/v1/event_pb";
import { LibraryService } from "@/gen/nama/v1/library_pb";
import type { ListUnmatchedItemsResponse } from "@/gen/nama/v1/match_pb";
import type { RouterContext } from "@/shared/hooks/access";
import { adminStopQuery } from "@/shared/hooks/admin-stop-query";
import { browseKey } from "@/shared/hooks/browse";
import { playsKey } from "@/shared/hooks/plays-key";
import { requestsKey } from "@/shared/hooks/requests-key";
import { scanQuery, scansKey } from "@/shared/hooks/scan-query";
import { searchKey } from "@/shared/hooks/search-query";
import { serverQueries } from "@/shared/hooks/server-key";
import { unmatchedKey } from "@/shared/hooks/unmatched-key";

/** Updates or invalidates the queries of the Server `response` came from that it's about. */
export function applyEvent({ message }: SubscribeResponse, context: RouterContext) {
  if (message.case === "event") {
    recordScan(message.value, context);
    relistUnmatched(message.value, context);
    dropDecided(message.value, context);
    refreshBrowsing(message.value, context);
    refreshLibrary(message.value, context);
    refreshRequests(message.value, context);
    refreshInbox(message.value, context);
    refreshPlays(message.value, context);
    recordAdminStop(message.value, context);
  } else if (message.case === "lagged") {
    forgetMissed(context);
  }
}

/**
 * Refetches everything of the Server after its Events were missed. Scans can't be refetched, so
 * what's known of them is forgotten rather than left stuck mid-scan.
 */
export function forgetMissed({ queryClient, transport }: RouterContext) {
  void queryClient.resetQueries({ queryKey: scansKey(transport) });
  void queryClient.invalidateQueries(serverQueries(transport));
}

function recordScan({ event }: Event, { queryClient, transport }: RouterContext) {
  if (
    event.case === "scanStarted" ||
    event.case === "scanProgress" ||
    event.case === "scanFinished"
  ) {
    queryClient.setQueryData(scanQuery(transport, event.value.libraryId).queryKey, event.value);
  }
}

/**
 * Lists the Items a finished scan left Unmatched, and again once Matching finishes: why it left
 * each one Unmatched is stored only then.
 */
function relistUnmatched({ event }: Event, { queryClient, transport }: RouterContext) {
  if (event.case === "scanFinished" || event.case === "matchingFinished") {
    const queryKey = unmatchedKey(transport, event.value.libraryId);
    void queryClient.invalidateQueries({ queryKey });
  }
}

/** Drops each Unmatched Item that is Matched or gone. */
function dropDecided({ event }: Event, context: RouterContext) {
  if (event.case === "itemMatched" || event.case === "libraryItemRemoved") {
    dropUnmatched(context, event.value.libraryId, event.value.itemId);
  }
}

/**
 * Takes the Item `itemId` off its Library's Unmatched Items at once, then refetches them. That
 * also restarts a refetch begun before this Event, whose answer would bring the Item back.
 */
// ponytail: one refetch per Event while the list is on screen; debounce if a big scan's burst of
// Matches turns out to load the Server.
function dropUnmatched(
  { queryClient, transport }: RouterContext,
  libraryId: bigint,
  itemId: bigint,
) {
  const queryKey = unmatchedKey(transport, libraryId);
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
function refreshBrowsing({ event }: Event, { queryClient, transport }: RouterContext) {
  if (
    event.case === "itemMatched" ||
    event.case === "libraryItemRemoved" ||
    event.case === "scanFinished"
  ) {
    // ponytail: every browse query, not only those of the Event's Library. Only the ones on screen
    // refetch, and a household Server has a handful of Libraries.
    void queryClient.invalidateQueries({ queryKey: browseKey(transport) });
  }
}

/** Refetches a Library when its scan finishes, which records the folders that look empty. */
function refreshLibrary({ event }: Event, { queryClient, transport }: RouterContext) {
  if (event.case !== "scanFinished") {
    return;
  }
  const input = { id: event.value.libraryId };
  const method = LibraryService.method.getLibrary;
  const queryKey = createConnectQueryKey({
    schema: method,
    input,
    transport,
    cardinality: "finite",
  });
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
function refreshRequests({ event }: Event, { queryClient, transport }: RouterContext) {
  if (requestEvents.has(event.case)) {
    void queryClient.invalidateQueries({ queryKey: requestsKey(transport) });
    void queryClient.invalidateQueries({ queryKey: searchKey(transport) });
  }
}

/**
 * Recounts the inbox on the Request events a notification comes with: a new Pending Request, a
 * change of state and a cancelling. Not on download progress, which ticks about once a minute.
 * The Server may store the notification just after sending its event, so this can miss it until
 * the next event or focus (#2309).
 */
function refreshInbox({ event }: Event, { queryClient, transport }: RouterContext) {
  if (
    event.case === "requestCreated" ||
    event.case === "requestStateChanged" ||
    event.case === "requestCancelled"
  ) {
    void queryClient.invalidateQueries({ queryKey: inboxKey(transport) });
  }
}

/**
 * Refetches the plays when one starts, changes or ends. The events don't carry a play's title,
 * progress or where it Transcodes, so the list is read again rather than patched by plan id.
 */
function refreshPlays({ event }: Event, { queryClient, transport }: RouterContext) {
  if (
    event.case === "playbackStarted" ||
    event.case === "playbackChanged" ||
    event.case === "playbackEnded"
  ) {
    void queryClient.invalidateQueries({ queryKey: playsKey(transport) });
  }
}

/** Keeps an Admin's stop of a plan, so its player closes, wherever it plays. */
function recordAdminStop({ event }: Event, { queryClient, transport }: RouterContext) {
  if (event.case === "playbackEnded" && event.value.playback && event.value.adminStop) {
    const { queryKey } = adminStopQuery(transport, event.value.playback.planId);
    queryClient.setQueryData(queryKey, event.value.adminStop);
  }
}
