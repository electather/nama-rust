import { Candidates } from "@/shared/components/find-a-match/candidates";
import { useSearchTitles } from "@/shared/hooks/use-search-titles";
import { type ToMatch, searchRefusal, type searchTyped } from "@/shared/lib/find-a-match";

/** The titles the Library's title sources found for `search`, to Match `item` to. */
export function SearchResults({
  search,
  ...toMatch
}: ToMatch & { search: ReturnType<typeof searchTyped> }) {
  const found = useSearchTitles({ ...search, libraryId: toMatch.libraryId });
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
  return <Candidates candidates={candidates} {...toMatch} />;
}
