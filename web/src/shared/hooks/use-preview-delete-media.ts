import { useQuery } from "@connectrpc/connect-query";

import { LibraryService } from "@/gen/nama/v1/library_pb";
import { type Deletable, targetOf } from "@/shared/lib/deletable";

/** What the Server says deleting `chosen` of `deletable` would take. */
export function usePreviewDeleteMedia(deletable: Deletable, chosen: string) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the dialog says it's finding the files, or why it couldn't, and stays up while it loads
  return useQuery(LibraryService.method.previewDeleteMedia, {
    target: targetOf(deletable, chosen),
  });
}
