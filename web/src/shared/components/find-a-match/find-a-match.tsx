import { useState } from "react";

import { SearchForm } from "@/shared/components/find-a-match/search-form";
import { SearchResults } from "@/shared/components/find-a-match/search-results";
import type { ItemToMatch } from "@/shared/lib/unmatched-names";

/** Searches the title source for `item`'s title, and Matches it to the title picked. */
export function FindAMatch({ item, libraryId }: { item: ItemToMatch; libraryId: bigint }) {
  const [search, setSearch] = useState({ kind: item.kind, title: item.title, year: item.year });
  return (
    <div className="flex max-w-xl flex-col gap-4">
      <SearchForm item={item} onSearch={setSearch} />
      <SearchResults search={search} item={item} libraryId={libraryId} />
    </div>
  );
}
