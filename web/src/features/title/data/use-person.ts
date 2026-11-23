import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetPersonRequestSchema } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** A Person, with the titles on this Server they have a Role in. */
export function usePerson(input: MessageInitShape<typeof GetPersonRequestSchema>) {
  return useSuspenseQuery(BrowseService.method.getPerson, input, { staleTime: loaderStaleTime })
    .data;
}
