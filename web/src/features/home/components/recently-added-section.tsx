import { useId } from "react";

import { NearScreen } from "@/features/home/components/near-screen";
import { NewestMovies } from "@/features/home/components/newest-movies";
import { NewestShows } from "@/features/home/components/newest-shows";
import { RecentlyAddedFrame } from "@/features/home/components/recently-added-frame";
import { RowLoader } from "@/features/home/components/row-loader";
import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { posterTile } from "@/features/home/lib/shelf-tiles";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";
import { LibraryKind } from "@/gen/nama/v1/library_pb";

/** A Library's Recently Added row, loading on its own once you scroll near it. */
export function RecentlyAddedSection({ row }: { row: RecentlyAddedHomeRow }) {
  const headingId = useId();
  const loading = (
    <RecentlyAddedFrame row={row} headingId={headingId}>
      <ShelfSkeleton tile={posterTile} aspect="aspect-2/3" />
    </RecentlyAddedFrame>
  );
  const Newest = row.libraryKind === LibraryKind.SHOWS ? NewestShows : NewestMovies;
  return (
    <NearScreen placeholder={loading}>
      <RowLoader loading={loading}>
        <Newest row={row} headingId={headingId} />
      </RowLoader>
    </NearScreen>
  );
}
