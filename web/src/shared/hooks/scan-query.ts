import { queryOptions, skipToken } from "@tanstack/react-query";

import type { ScanFinished, ScanProgress, ScanStarted } from "@/gen/nama/v1/event_pb";

/** The latest scan Event about a Library, if one arrived since the page was opened. */
export type ScanState = ScanStarted | ScanProgress | ScanFinished;

/** The query key every Library's latest scan Event is kept under. */
export const SCANS = "scan";

/** Where the latest scan Event about the Library `libraryId` is kept. Only Events fill it. */
export function scanQuery(libraryId: bigint) {
  return queryOptions<ScanState>({ queryKey: [SCANS, String(libraryId)], queryFn: skipToken });
}
