import { PosterShelf } from "@/features/home/components/poster-shelf";
import { RecentlyAddedFrame } from "@/features/home/components/recently-added-frame";
import type { ShelfTitles } from "@/features/home/lib/shelf-tiles";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Library's Recently Added row of posters, `newest`, or nothing if the Library has no titles. */
export function RecentlyAddedShelf({
  row,
  headingId,
  newest,
}: {
  row: RecentlyAddedHomeRow;
  headingId: string;
  newest: ShelfTitles;
}) {
  if (newest.titles.length === 0 && !newest.next.has) {
    return null;
  }
  return (
    <RecentlyAddedFrame row={row} headingId={headingId}>
      <PosterShelf {...newest} />
    </RecentlyAddedFrame>
  );
}
