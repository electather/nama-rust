import type { ReactNode } from "react";

import { MovieCounts } from "@/features/library/components/movie-counts";
import { ShowCounts } from "@/features/library/components/show-counts";
import type { TitleListing } from "@/features/library/lib/title-listing";

/** What `children` shows of how many Movies or Shows (`kind`) `listing` keeps, and in all. */
export function TitleCounts(props: {
  libraryId: bigint;
  listing: TitleListing;
  kind: "movies" | "shows";
  children: (counts: { shown: number | undefined; all: number | undefined }) => ReactNode;
}) {
  return props.kind === "movies" ? <MovieCounts {...props} /> : <ShowCounts {...props} />;
}
