import { VirtualPosterGrid } from "@/features/library/components/virtual-poster-grid";
import type { GridProps } from "@/features/library/lib/titles-props";
import { ShowMore } from "@/shared/components/show-more";

/** The posters, and Show more under them while a page follows, or that none match the filters. */
export function TitleGrid({ titles, paged, posters }: GridProps) {
  if (posters.length === 0) {
    return <p className="mt-8 text-muted-foreground">No {titles.many} match these filters.</p>;
  }
  return (
    <>
      <VirtualPosterGrid posters={posters} setSize={paged.totalSize} />
      {paged.hasNextPage && (
        <ShowMore load={paged.fetchNextPage} loaded={paged.data.pages.length} />
      )}
    </>
  );
}
