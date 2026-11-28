import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
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
    return <p className="text-sm text-muted-foreground">Type a title to search for.</p>;
  }
  if (found.data === undefined) {
    return <p className="text-sm text-muted-foreground">Searching…</p>;
  }
  if (found.candidates.length === 0) {
    return (
      <p className="text-sm text-muted-foreground">Nothing found. Try another title or year.</p>
    );
  }
  return <Candidates candidates={found.candidates} picked={picked} onPick={onPick} />;
}
