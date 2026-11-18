import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type ListEpisodesRequestSchema } from "@/gen/nama/v1/browse_pb";

/** The Episodes of one Season of a Show. */
export function useEpisodes(input: MessageInitShape<typeof ListEpisodesRequestSchema>) {
  return useSuspenseQuery(BrowseService.method.listEpisodes, input).data;
}
