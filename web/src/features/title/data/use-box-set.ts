import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetBoxSetRequestSchema } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** A Box Set, with its Movies as you see them. */
export function useBoxSet(input: MessageInitShape<typeof GetBoxSetRequestSchema>) {
  return useSuspenseQuery(BrowseService.method.getBoxSet, input, { staleTime: loaderStaleTime })
    .data;
}
