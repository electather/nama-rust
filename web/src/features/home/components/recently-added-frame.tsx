import type { ReactNode } from "react";

import { RowFrame } from "@/features/home/components/row-frame";
import type { RecentlyAddedHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Library's Recently Added row under its heading, which opens the Library newest first. */
export function RecentlyAddedFrame({
  row,
  headingId,
  children,
}: {
  row: RecentlyAddedHomeRow;
  headingId: string;
  children: ReactNode;
}) {
  return (
    <RowFrame
      heading={`Recently added in ${row.libraryName}`}
      headingId={headingId}
      link={{
        to: "/libraries/$libraryId",
        params: { libraryId: String(row.libraryId) },
        search: { sort: "added" },
      }}
    >
      {children}
    </RowFrame>
  );
}
