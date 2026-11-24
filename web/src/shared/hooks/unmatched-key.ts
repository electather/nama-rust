import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { MatchService } from "@/gen/nama/v1/match_pb";

/**
 * The query key of the Library `libraryId`'s Unmatched Items on the Server `transport` talks to, or
 * every Library's without one.
 */
export function unmatchedKey(transport: Transport, libraryId?: bigint) {
  return createConnectQueryKey({
    schema: MatchService.method.listUnmatchedItems,
    ...(libraryId !== undefined && { input: { filter: { libraryId } } }),
    transport,
    cardinality: "infinite",
  });
}
