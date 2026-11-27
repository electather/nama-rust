import { cn } from "cn";
import { ghostHang } from "nama-ui/components/button";

import { FilterPopover } from "@/features/library/components/filter-popover";
import { FilterTokens } from "@/features/library/components/filter-tokens";
import { SortMenu } from "@/features/library/components/sort-menu";
import type { BarProps } from "@/features/library/lib/listing-bar";

/**
 * The bar over a Library's grid: the sort and its direction, the Filter button for any of
 * `genres`, a Watch State and years, then a chip for each filter that's on. `change` gets the
 * listing with what the User changed.
 */
export function TitleListControls({ genres, ...bar }: BarProps & { genres: string[] }) {
  return (
    <div className="mt-6">
      <div className={cn(ghostHang, "flex flex-wrap items-center gap-1")}>
        <SortMenu {...bar} />
        <FilterPopover genres={genres} {...bar} />
      </div>
      <FilterTokens {...bar} />
    </div>
  );
}
