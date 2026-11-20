import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetMovieRequestSchema } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** A Movie of a Library, with its Versions, and whether its folder is read-only. */
export function useMovie(input: MessageInitShape<typeof GetMovieRequestSchema>) {
  return useSuspenseQuery(BrowseService.method.getMovie, input, { staleTime: loaderStaleTime })
    .data;
}
