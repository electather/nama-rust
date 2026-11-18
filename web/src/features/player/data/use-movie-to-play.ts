import type { MessageInitShape } from "@bufbuild/protobuf";
import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService, type GetMovieRequestSchema } from "@/gen/nama/v1/browse_pb";

/** The Movie to play, and the error of the last refetch, if it failed. */
export function useMovieToPlay(input: MessageInitShape<typeof GetMovieRequestSchema>) {
  const { data, error } = useSuspenseQuery(BrowseService.method.getMovie, input);
  return { data, error };
}
