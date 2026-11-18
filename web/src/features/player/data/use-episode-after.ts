import type { MessageInitShape } from "@bufbuild/protobuf";
import { useQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetNextEpisodeRequestSchema } from "@/gen/nama/v1/browse_pb";

/** The Episode after `episode`, once the Server has found it. */
export function useEpisodeAfter(episode: MessageInitShape<typeof GetNextEpisodeRequestSchema>) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Episode plays while the Server finds the next one
  return useQuery(BrowseService.method.getNextEpisode, episode).data?.next;
}
