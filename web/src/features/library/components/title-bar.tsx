import type { ReactNode } from "react";

import { FilterTokens } from "@/features/library/components/filter-tokens";
import { TitleControls } from "@/features/library/components/title-controls";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/**
 * The row over a Library's grid: `start`, such as the Movies and Box Sets tabs, then the sort and
 * Filter for `titles`, and under it a chip for each filter that's on. A Library with no titles
 * has nothing to sort.
 */
export function TitleBar({
  start,
  titles,
}: {
  start?: ReactNode;
  titles: (TitlesProps & { kind: "movies" | "shows" }) | undefined;
}) {
  return (
    <div className="flex flex-col gap-3.5">
      <div className="flex flex-wrap items-center justify-between gap-x-5 gap-y-2.5">
        {start}
        {titles && <TitleControls {...titles} />}
      </div>
      {titles && <FilterTokens listing={titles.listing} change={titles.change} />}
    </div>
  );
}
