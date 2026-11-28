import type { ExternalId } from "@/gen/nama/v1/match_pb";
import { titleParam } from "@/shared/lib/title-param";

/** A Show by its Library and its id, as the Show page hands it to the cards of its row. */
export interface ShowInLibrary {
  libraryId: bigint;
  showId: ExternalId;
}

/** The URL params of `show`'s page, and of the pages under it. */
export function showParams({ libraryId, showId }: ShowInLibrary) {
  return { libraryId: String(libraryId), showId: titleParam(showId) };
}
