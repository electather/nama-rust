import { NotMatchedRow } from "@/features/library/components/not-matched-row";
import { UnmatchedCount } from "@/features/library/components/unmatched-count";
import { firstUnmatchedPage } from "@/features/library/lib/first-unmatched-page";
import { BrowseService } from "@/gen/nama/v1/browse_pb";
import { ShowMore } from "@/shared/components/show-more";
import { usePaged } from "@/shared/hooks/use-paged";

/**
 * The Library `libraryId`'s Unmatched Items, folded away at the foot of its page by what their
 * names say, never where their files are (ADR 0069). Nothing if it has none.
 */
export function NotMatched({ libraryId }: { libraryId: bigint }) {
  const { data, hasNextPage, fetchNextPage, totalSize } = usePaged(
    BrowseService.method.listUnmatched,
    firstUnmatchedPage(libraryId),
  );
  const items = data.pages.flatMap((page) => page.items);
  if (items.length === 0 && !hasNextPage) {
    return null;
  }
  return (
    <details className="mt-12 max-w-3xl">
      <summary className="cursor-pointer font-medium">
        Not matched
        <UnmatchedCount count={totalSize} />
      </summary>
      <p className="mt-2 max-w-prose text-sm text-pretty text-muted-foreground">
        The Server couldn’t tell which Movie or Episode these files are.
      </p>
      <ul className="mt-2 flex flex-col">
        {items.map((item) => (
          // A file holding two Episodes lists each, under the same Version.
          <NotMatchedRow
            key={`${item.versionId}:${item.episode}`}
            item={item}
            libraryId={libraryId}
          />
        ))}
      </ul>
      {hasNextPage && <ShowMore load={fetchNextPage} loaded={data.pages.length} />}
    </details>
  );
}
