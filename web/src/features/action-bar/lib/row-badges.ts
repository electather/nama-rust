import type { Row } from "@/features/action-bar/lib/rows";
import type { FoundTitle } from "@/features/action-bar/lib/titles";
import { TitleStatus } from "@/gen/nama/v1/search_pb";
import { titleStanding } from "@/shared/lib/request-standing";

const badges = { movie: "Movie", show: "Show", episode: "Episode" } as const;

/** What a row of Recent says beside its label: its kind, as Recent mixes them. */
export function kindBadge(title: FoundTitle): Pick<Row, "badge" | "standing"> {
  return { badge: badges[title.kind] };
}

/**
 * What a search result says beside its label: nothing when Available, else its Request, or for a
 * Followed Show with no open Season Request, that it's Requested (ADR 0108).
 */
export function statusBadge({
  libraryId,
  request,
  status,
}: FoundTitle): Pick<Row, "badge" | "standing"> {
  const followed = status === TitleStatus.REQUESTED;
  const standing = titleStanding({ request, inLibrary: libraryId !== undefined, followed });
  if (standing.kind === "inLibrary") {
    return {};
  }
  if ("request" in standing) {
    return { standing };
  }
  return { badge: standing.kind === "followedOnly" ? "Requested" : "Request" };
}
