import { TitleCounts } from "@/features/library/components/title-counts";
import { countLine } from "@/features/library/lib/count-line";
import type { TitleListing } from "@/features/library/lib/title-listing";

const names = {
  movies: { one: "Movie", many: "Movies" },
  shows: { one: "Show", many: "Shows" },
};

/** "55 Movies", or "15 of 55 Movies" while filters keep only some. */
export function TitleCountLine(props: {
  libraryId: bigint;
  listing: TitleListing;
  kind: "movies" | "shows";
}) {
  return <TitleCounts {...props}>{(counts) => countLine(counts, names[props.kind])}</TitleCounts>;
}
