import { Search, SearchX } from "lucide-react";

import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { EmptyNote } from "@/shared/components/empty-note";
import { Candidates } from "@/shared/components/find-a-match/candidates";
import type { useFindAMatch } from "@/shared/hooks/use-find-a-match";
import { searchRefusal } from "@/shared/lib/find-a-match";

/**
 * The titles `found` by the search, to pick one of, with `picked` lit; or that it's searching, why
 * it failed, or that it found none.
 */
export function SearchResults({
  found,
  picked,
  onPick,
}: {
  found: ReturnType<typeof useFindAMatch>["found"];
  picked: TitleCandidate | undefined;
  onPick: (key: string) => void;
}) {
  if (found.isError) {
    return (
      <p role="alert" className="text-sm text-destructive">
        {searchRefusal(found.error)}
      </p>
    );
  }
  if (found.blank) {
    return (
      <EmptyNote Icon={Search} title="No title to search for" todo="Type the title this file is." />
    );
  }
  if (found.data === undefined) {
    return <p className="text-sm text-muted-foreground">Searching…</p>;
  }
  if (found.candidates.length === 0) {
    return <EmptyNote Icon={SearchX} title="Nothing found" todo="Try another title or year." />;
  }
  return <Candidates candidates={found.candidates} picked={picked} onPick={onPick} />;
}
