import { useSuspenseQuery } from "@connectrpc/connect-query";

import { BrowseService } from "@/gen/nama/v1/browse_pb";

/** The genres the Library `libraryId`'s titles have, for its genre filter. */
export function useGenres(libraryId: bigint) {
  return useSuspenseQuery(BrowseService.method.listGenres, { libraryId }).data.genres;
}
