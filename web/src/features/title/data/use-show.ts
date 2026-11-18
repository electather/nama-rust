import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetShowRequestSchema } from "@/gen/nama/v1/browse_pb";

/** A Show of a Library, with its Seasons, and the error of the last refetch, if it failed. */
export function useShow(input: MessageInitShape<typeof GetShowRequestSchema>) {
  const { data, error } = useSuspenseQuery(BrowseService.method.getShow, input);
  return { data, error };
}
