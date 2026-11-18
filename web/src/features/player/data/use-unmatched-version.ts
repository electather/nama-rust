import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetUnmatchedVersionRequestSchema } from "@/gen/nama/v1/browse_pb";

/** An Unmatched Item and the Version to play, and the error of the last refetch, if it failed. */
export function useUnmatchedVersion(
  input: MessageInitShape<typeof GetUnmatchedVersionRequestSchema>,
) {
  const { data, error } = useSuspenseQuery(BrowseService.method.getUnmatchedVersion, input);
  return { data, error };
}
