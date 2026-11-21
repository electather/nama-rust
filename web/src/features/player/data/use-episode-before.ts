import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetPreviousEpisodeRequestSchema } from "@/gen/nama/v1/browse_pb";

/** The Episode before `episode`, once the Server has found it. */
export function useEpisodeBefore(
  episode: MessageInitShape<typeof GetPreviousEpisodeRequestSchema>,
) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Episode plays while the Server finds the previous one
  return useQuery(BrowseService.method.getPreviousEpisode, episode).data?.previous;
}
