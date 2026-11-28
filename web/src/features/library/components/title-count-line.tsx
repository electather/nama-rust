import { TitleCounts } from "@/features/library/components/title-counts";
import { UnmatchedCountLine } from "@/features/library/components/unmatched-count-line";
import { countLine } from "@/features/library/lib/count-line";
import { isUnmatched, type TitleListing } from "@/features/library/lib/title-listing";

const names = {
  movies: { one: "Movie", many: "Movies" },
  shows: { one: "Show", many: "Shows" },
};

/** "55 Movies", or "15 of 55 Movies" while filters keep only some, or "3 Not matched" while it lists only those. */
export function TitleCountLine(props: {
  libraryId: bigint;
  listing: TitleListing;
  kind: "movies" | "shows";
}) {
  if (isUnmatched(props.listing)) {
    return <UnmatchedCountLine libraryId={props.libraryId} />;
  }
  return <TitleCounts {...props}>{(counts) => countLine(counts, names[props.kind])}</TitleCounts>;
}
