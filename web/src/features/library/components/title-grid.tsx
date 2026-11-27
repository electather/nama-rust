import { NoMatches } from "@/features/library/components/no-matches";
import { VirtualPosterGrid } from "@/features/library/components/virtual-poster-grid";
import type { GridProps, Jump } from "@/features/library/lib/titles-props";
import { ShowMore } from "@/shared/components/show-more";

/**
 * The posters, and Show more under them while a page follows, or that none match the filters,
 * with `clear` to clear them.
 */
export function TitleGrid({ titles, paged, posters, clear, jump }: GridProps & { jump?: Jump }) {
  if (posters.length === 0) {
    return <NoMatches titles={titles.many} clear={clear} />;
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
