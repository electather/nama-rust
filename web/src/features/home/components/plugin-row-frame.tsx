import type { LinkOptions } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { RowFrame } from "@/features/home/components/row-frame";
import type { PluginHomeRow } from "@/gen/nama/v1/browse_pb";

/** A Plugin's row under its heading, which opens the Library the Plugin linked it to, if any. */
export function PluginRowFrame({
  row,
  headingId,
  children,
}: {
  row: PluginHomeRow;
  headingId: string;
  children: ReactNode;
}) {
  const link =
    row.libraryId === undefined
      ? undefined
      : ({
          to: "/libraries/$libraryId",
          params: { libraryId: String(row.libraryId) },
        } satisfies LinkOptions);
  return (
    <RowFrame heading={row.heading} headingId={headingId} link={link}>
      {children}
    </RowFrame>
  );
}
