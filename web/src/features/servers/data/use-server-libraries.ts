import type { Transport } from "@connectrpc/connect";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";

/** The Libraries of the Server `transport` reaches. */
export function useServerLibraries(transport: Transport) {
  return useSuspenseQuery(LibraryService.method.listLibraries, {}, { transport }).data.libraries;
}
