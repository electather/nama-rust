import { Candidates } from "@/shared/components/find-a-match/candidates";
import { useSearchTitles } from "@/shared/hooks/use-search-titles";
import { searchRefusal, type searchTyped } from "@/shared/lib/find-a-match";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/** The titles the title source found for `search`, to Match `item` to. */
export function SearchResults({
  search,
  item,
  libraryId,
}: {
  search: ReturnType<typeof searchTyped>;
  item: ItemToMatch;
  libraryId: bigint;
}) {
  const found = useSearchTitles(search);
  if (found.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {searchRefusal(found.error)}
      </p>
    );
  }
  if (found.data === undefined) {
    return <p className="text-sm text-muted-foreground">Searching…</p>;
  }
  const { candidates } = found.data;
  if (candidates.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">Nothing found. Try another title or year.</p>
    );
  }
  return <Candidates candidates={candidates} item={item} libraryId={libraryId} />;
}
