import { RecentlyAddedShelf } from "@/features/home/components/recently-added-shelf";
import { useNewestMovies } from "@/features/home/data/use-newest-movies";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Movies Library's Recently Added row once its Movies load. */
export function NewestMovies({ row, headingId }: { row: RecentlyAddedHomeRow; headingId: string }) {
  return (
    <RecentlyAddedShelf row={row} headingId={headingId} newest={useNewestMovies(row.libraryId)} />
  );
}
