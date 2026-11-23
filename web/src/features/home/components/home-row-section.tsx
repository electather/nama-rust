import { useId } from "react";

import { NearScreen } from "@/features/home/components/near-screen";
import { RowFrame } from "@/features/home/components/row-frame";
import { RowLoader } from "@/features/home/components/row-loader";
import { RowShelf } from "@/features/home/components/row-shelf";
import { ShelfSkeleton } from "@/features/home/components/shelf-skeleton";
import { posterTile } from "@/features/home/lib/shelf-tiles";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Plugin's row, loading on its own: at once when `eager`, else once you scroll near it. */
export function HomeRowSection({ row, eager }: { row: PluginHomeRow; eager: boolean }) {
  const headingId = useId();
  const loading = (
    <RowFrame heading={row.heading} headingId={headingId}>
      <ShelfSkeleton tile={posterTile} aspect="aspect-2/3" />
    </RowFrame>
  );
  const shelf = (
    <RowLoader loading={loading}>
      <RowShelf row={row} headingId={headingId} />
    </RowLoader>
  );
  return eager ? shelf : <NearScreen placeholder={loading}>{shelf}</NearScreen>;
}
