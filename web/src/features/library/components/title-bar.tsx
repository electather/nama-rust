import type { ReactNode } from "react";

import { TitleControls } from "@/features/library/components/title-controls";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/**
 * The row over a Library's grid: `start`, such as the Movies and Box Sets tabs, then, at its end,
 * the sort and Filter for `titles`. A Library with no titles has nothing to sort.
 */
export function TitleBar({
  start,
  titles,
}: {
  start?: ReactNode;
  titles: (TitlesProps & { kind: "movies" | "shows" }) | undefined;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-5 gap-y-2.5">
      {start}
      {titles && <TitleControls {...titles} />}
    </div>
  );
}
