import { Button } from "nama-ui/components/button";
import { Empty } from "nama-ui/components/empty";

import { NoMatchesHeader } from "@/features/library/components/no-matches-header";

/** What a Library's grid says when no `titles` pass its filters, with `clear` to clear them. */
export function NoMatches({ titles, clear }: { titles: string; clear: (() => void) | undefined }) {
  return (
    <Empty>
      <NoMatchesHeader titles={titles} />
      {clear && (
        <Button variant="secondary" onClick={clear}>
          Clear filters
        </Button>
      )}
    </Empty>
  );
}
