import { Button } from "nama-ui/components/button";

import { VirtualPosterGrid } from "@/features/library/components/virtual-poster-grid";
import type { GridProps, Jump } from "@/features/library/lib/titles-props";
import { ShowMore } from "@/shared/components/show-more";

/**
 * The posters, and Show more under them while a page follows, or that none match the filters,
 * with `clear` to clear them.
 */
export function TitleGrid({ titles, paged, posters, clear, jump }: GridProps & { jump?: Jump }) {
  if (posters.length === 0) {
    return (
      <div className="flex flex-col items-start gap-2.5 py-12">
        <p className="font-medium">No {titles.many} match these filters</p>
        {clear && (
          <Button variant="secondary" onClick={clear}>
            Clear filters
          </Button>
        )}
      </div>
    );
  }
  return (
    <>
      <VirtualPosterGrid posters={posters} setSize={paged.totalSize} jump={jump} />
      {paged.hasNextPage && (
        <ShowMore load={paged.fetchNextPage} loaded={paged.data.pages.length} />
      )}
    </>
  );
}
