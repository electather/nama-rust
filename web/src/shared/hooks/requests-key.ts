import { createConnectQueryKey } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** Every Request query, including the Movies found to Request: what to refresh when one changes. */
// ponytail: refreshing a search, Movie or Show page on screen asks TMDB again, once per Request
// Event, and a Show page once more per Season. That includes progress, about once a minute per
// Processing or Partially Available Request while its downloads move. Patch the found title from
// the Event's Request instead if that load shows.
export const requestsKey = createConnectQueryKey({
  schema: RequestService,
  cardinality: undefined,
});
