import { RecentlyAddedShelf } from "@/features/home/components/recently-added-shelf";
import { useNewestShows } from "@/features/home/data/use-newest-shows";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Shows Library's Recently Added row once its Shows load. */
export function NewestShows({ row, headingId }: { row: RecentlyAddedHomeRow; headingId: string }) {
  return (
    <RecentlyAddedShelf row={row} headingId={headingId} newest={useNewestShows(row.libraryId)} />
  );
}
