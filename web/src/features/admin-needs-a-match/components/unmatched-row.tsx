import { useId } from "react";

import { ItemFacts } from "@/features/admin-needs-a-match/components/item-facts";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { FindAMatch } from "@/shared/components/find-a-match/find-a-match";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { parsedAs } from "@/shared/lib/unmatched-names";
import { Button } from "@/shared/ui/button";

/**
 * An Unmatched Item: what its file's name was parsed as, and the file. Opening it searches the
 * title source for its title, to Match it by hand.
 */
export function UnmatchedRow({
  item,
  libraryId,
  open,
  onToggle,
}: {
  item: UnmatchedItem;
  libraryId: bigint;
  open: boolean;
  onToggle: () => void;
}) {
  const panel = useId();
  const parsed = parsedAs(item);
  const action = open ? "Close" : "Find a Match";
  return (
    <li className="border-b p-3 last:border-b-0">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <ItemFacts item={item} parsed={parsed} />
        <PlayUnmatched libraryId={libraryId} versionId={item.versionId} name={parsed} />
        <Button
          variant="ghost"
          aria-label={`${action} for ${parsed}`}
          aria-expanded={open}
          aria-controls={panel}
          onClick={onToggle}
        >
          {action}
        </Button>
      </div>
      {open && (
        <div id={panel} className="pt-4 pb-3">
          <FindAMatch item={item} libraryId={libraryId} />
        </div>
      )}
    </li>
  );
}
