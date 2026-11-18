import { ShowMore } from "@/components/show-more";
import { PlayUnmatched, parsedAs } from "@/components/unmatched-item";
import { BrowseService, type UnmatchedItemSummary } from "@/gen/nama/v1/browse_pb";
import { usePaged } from "@/lib/paged";

/** `ListUnmatched`'s first page of the Library `libraryId`'s Unmatched Items. */
export function firstUnmatchedPage(libraryId: bigint) {
  return { libraryId, page: { token: "" } };
}

/**
 * The Library `libraryId`'s Unmatched Items, folded away at the foot of its page by what their
 * names say, never where their files are (ADR 0062). Nothing if it has none.
 */
export function NotMatched({ libraryId }: { libraryId: bigint }) {
  const { data, hasNextPage, fetchNextPage, totalSize } = usePaged(
    BrowseService.method.listUnmatched,
    firstUnmatchedPage(libraryId),
  );
  const items = data.pages.flatMap((page) => page.items);
  if (items.length === 0 && !hasNextPage) return null;
  return (
    <details className="mt-12 max-w-3xl">
      <summary className="cursor-pointer font-medium">
        Not matched
        <Count count={totalSize} />
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

/** How many Items it lists, once the first page has said. */
function Count({ count }: { count: number | undefined }) {
  if (count === undefined) return null;
  return (
    <span className="ms-2 text-sm font-normal text-muted-foreground">{count.toLocaleString()}</span>
  );
}

function NotMatchedRow({ item, libraryId }: { item: UnmatchedItemSummary; libraryId: bigint }) {
  const name = parsedAs(item);
  return (
    <li className="flex items-center gap-3 border-b py-2 last:border-b-0">
      <span className="min-w-0 flex-1">{name}</span>
      <PlayUnmatched libraryId={libraryId} versionId={item.versionId} name={name} />
    </li>
  );
}
