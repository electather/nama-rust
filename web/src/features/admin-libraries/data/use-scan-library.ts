import { useMutation } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";

/** Starts scanning a Library's folders. Its progress arrives as Events. */
export function useScanLibrary() {
  return useMutation(LibraryService.method.scanLibrary);
}
