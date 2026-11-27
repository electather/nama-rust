import type { ReactNode } from "react";

import { useShowCounts } from "@/features/library/data/use-title-counts";
import type { TitleListing } from "@/features/library/lib/title-listing";

/** What `children` shows of how many Shows `listing` keeps, and how many the Library has. */
export function ShowCounts({
  libraryId,
  listing,
  children,
}: {
  libraryId: bigint;
  listing: TitleListing;
  children: (counts: { shown: number | undefined; all: number | undefined }) => ReactNode;
}) {
  return <>{children(useShowCounts(libraryId, listing))}</>;
}
