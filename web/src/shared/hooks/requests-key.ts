import { createConnectQueryKey } from "@connectrpc/connect-query";

import { RequestService } from "@/gen/nama/v1/request_pb";

/** Every Request query: what to refresh, with `searchKey`'s searches, when one changes. */
// ponytail: refreshing a search, Movie or Show page on screen asks TMDB again, once per Request
// Event, and a Show page once more per Season. That includes progress, about once a minute per
// Processing or Partially Available Request while its downloads move. Patch the found title from
// the Event's Request instead if that load shows.
export const requestsKey = createConnectQueryKey({
  schema: RequestService,
  cardinality: undefined,
});
