import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type ListEpisodesRequestSchema } from "@/gen/nama/v1/browse_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** The Episodes of a Season, one of which plays, and the error of the last refetch, if it failed. */
export function useEpisodesToPlay(input: MessageInitShape<typeof ListEpisodesRequestSchema>) {
  const { data, error } = useSuspenseQuery(BrowseService.method.listEpisodes, input, {
    staleTime: loaderStaleTime,
  });
  return { data, error };
}
