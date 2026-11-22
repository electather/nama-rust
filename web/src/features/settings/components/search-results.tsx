import { SearchResult } from "@/features/settings/components/search-result";
import type { SearchState } from "@/features/settings/hooks/use-settings-search";
import { findableId } from "@/features/settings/lib/setting-search";

/**
 * What Settings' search found, as the field's listbox `listId`, there even while empty so the field
 * always names it, and under it that nothing matches.
 */
export function SearchResults({ search, listId }: { search: SearchState; listId: string }) {
  const { typed, results, picked } = search;
  const nothing = (
    <p className="px-4 text-sm text-pretty text-muted-foreground md:px-2">
      No settings match “{typed.trim()}”.
    </p>
  );
  const list = (
    <div
      id={listId}
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- a <select> can't leave focus in the field while its options are picked
      role="listbox"
      aria-label="Settings found"
      tabIndex={-1}
      // A press here mustn't take focus from the field before the result opens.
      onMouseDown={(event) => {
        event.preventDefault();
      }}
      className="flex min-h-0 flex-col gap-0.5 overflow-y-auto overscroll-contain max-md:flex-1 max-md:px-2 max-md:pb-2"
    >
      {results.map((result, index) => (
        <SearchResult
          key={findableId(result)}
          id={`${listId}-${index}`}
          result={result}
          typed={typed}
          picked={result === picked}
          onOpen={search.handleOpen}
        />
      ))}
    </div>
  );
  return (
    <>
      {list}
      {results.length === 0 && nothing}
    </>
  );
}
