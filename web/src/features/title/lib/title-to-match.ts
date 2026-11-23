import type { ExternalId, TitleKind } from "@/gen/nama/v1/match_pb";
import type { TitleToMatch } from "@/shared/lib/find-a-match";
import { yearOf } from "@/shared/lib/title-facts";

/**
 * What Fix Match Matches again, `matchedTitle`, a Movie or Show as `kind` says, and what it searches
 * for first: its `title`, and the year of its `date`, such as a Movie's release date.
 */
export function titleToMatch(
  matchedTitle: { libraryId: bigint; id: ExternalId },
  kind: TitleKind,
  { title, date }: { title: string; date: string | undefined },
): TitleToMatch {
  const year = yearOf(date);
  return { kind, title, year: year === undefined ? undefined : Number(year), matchedTitle };
}
