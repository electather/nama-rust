import type { Transport } from "@connectrpc/connect";
import { queryOptions, skipToken } from "@tanstack/react-query";

import type { ScanFinished, ScanProgress, ScanStarted } from "@/gen/nama/v1/event_pb";
import { serverKey } from "@/shared/hooks/server-key";

/** The latest scan Event about a Library, if one arrived since the page was opened. */
export type ScanState = ScanStarted | ScanProgress | ScanFinished;

/** The query key every Library's latest scan Event on the Server `transport` talks to is kept under. */
export function scansKey(transport: Transport) {
  return ["scan", serverKey(transport)] as const;
}

/** Where the latest scan Event about the Library `libraryId` is kept. Only Events fill it. */
export function scanQuery(transport: Transport, libraryId: bigint) {
  return queryOptions<ScanState>({
    queryKey: [...scansKey(transport), String(libraryId)],
    queryFn: skipToken,
  });
}
