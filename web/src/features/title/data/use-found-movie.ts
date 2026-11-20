import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { type LookUpMovieRequestSchema, RequestService } from "@/gen/nama/v1/request_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** A Movie from the title source, for Requesting it. */
export function useFoundMovie(input: MessageInitShape<typeof LookUpMovieRequestSchema>) {
  return useSuspenseQuery(RequestService.method.lookUpMovie, input, { staleTime: loaderStaleTime })
    .data.movie;
}
