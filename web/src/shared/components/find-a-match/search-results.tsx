import { type LucideIcon, Search, SearchX } from "lucide-react";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "nama-ui/components/empty";

import type { TitleCandidate } from "@/gen/nama/v1/match_pb";
import { Candidates } from "@/shared/components/find-a-match/candidates";
import type { useFindAMatch } from "@/shared/hooks/use-find-a-match";
import { searchRefusal } from "@/shared/lib/find-a-match";

/** Says the search has nothing to pick from: its `Icon`, `title` and what to do. */
function nothing(Icon: LucideIcon, title: string, todo: string) {
  const icon = (
    <EmptyMedia>
      <Icon aria-hidden strokeWidth={1.5} />
    </EmptyMedia>
  );
  return (
    <Empty>
      <EmptyHeader>
        {icon}
        <EmptyTitle>{title}</EmptyTitle>
        <EmptyDescription>{todo}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}

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
    return nothing(Search, "No title to search for", "Type the title this file is.");
  }
  if (found.data === undefined) {
    return <p className="text-sm text-muted-foreground">Searching…</p>;
  }
  if (found.candidates.length === 0) {
    return nothing(SearchX, "Nothing found", "Try another title or year.");
  }
  return <Candidates candidates={found.candidates} picked={picked} onPick={onPick} />;
}
