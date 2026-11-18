import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery } from "@connectrpc/connect-query";

import { LibraryService, type ListArtworkRequestSchema } from "@/gen/nama/v1/library_pb";

/** The artwork the title sources list for a title, once it arrives. Images asking alike share one call. */
export function useArtwork(asked: MessageInitShape<typeof ListArtworkRequestSchema>) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the dialog shows the images at once, and each one's artwork when it arrives
  return useQuery(LibraryService.method.listArtwork, asked).data;
}
