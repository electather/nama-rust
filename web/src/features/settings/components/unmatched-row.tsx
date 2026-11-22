import { Film, Tv } from "lucide-react";
import { Button } from "nama-ui/components/button";
import { useId } from "react";

import { ItemFacts } from "@/features/admin-needs-a-match/components/item-facts";
import { ItemRow } from "@/features/settings/components/item-row";
import { TitleKind, type UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { FindAMatch } from "@/shared/components/find-a-match/find-a-match";
import { IconTile } from "@/shared/components/icon-tile";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * An Unmatched Item: what its file's name was parsed as, over the file's facts; then Play and
 * Find a Match, which searches the title source for its title under the row, to Match it by hand.
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
  const search = open && (
    <div id={panel} className="pt-2 pb-1">
      <FindAMatch item={item} libraryId={libraryId} />
    </div>
  );
  return (
    <ItemRow
      start={<IconTile Icon={item.kind === TitleKind.SHOW ? Tv : Film} />}
      name={parsed}
      meta={<ItemFacts item={item} />}
      under={search}
    >
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
    </ItemRow>
  );
}
