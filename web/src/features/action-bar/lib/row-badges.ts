import type { Row } from "@/features/action-bar/lib/rows";
import type { FoundTitle } from "@/features/action-bar/lib/titles";
import { TitleStatus } from "@/gen/nama/v1/search_pb";

const badges = { movie: "Movie", show: "Show", episode: "Episode" } as const;

/** What a row of Recent says beside its label: its kind, as Recent mixes them. */
export function kindBadge(title: FoundTitle): Pick<Row, "badge" | "request"> {
  return { badge: badges[title.kind] };
}

/**
 * What a search result says beside its label: nothing when Available, else its Request, or for a
 * Followed Show with no open Season Request, that it's Requested.
 */
export function statusBadge({
  libraryId,
  request,
  status,
}: FoundTitle): Pick<Row, "badge" | "request"> {
  if (libraryId !== undefined) {
    return {};
  }
  if (request !== undefined) {
    return { request };
  }
  return { badge: status === TitleStatus.REQUESTED ? "Requested" : "Request" };
}
