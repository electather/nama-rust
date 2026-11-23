import { PosterShelf } from "@/features/home/components/poster-shelf";
import { RecentlyAddedFrame } from "@/features/home/components/recently-added-frame";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";
import type { RowTitle } from "@/shared/lib/row-titles";

/** A Library's Recently Added row of posters, `newest`, or nothing if the Library has no titles. */
export function RecentlyAddedShelf({
  row,
  headingId,
  newest,
}: {
  row: RecentlyAddedHomeRow;
  headingId: string;
  newest: RowTitle[];
}) {
  if (newest.length === 0) {
    return null;
  }
  return (
    <RecentlyAddedFrame row={row} headingId={headingId}>
      <PosterShelf titles={newest} setSize={newest.length} />
    </RecentlyAddedFrame>
  );
}
