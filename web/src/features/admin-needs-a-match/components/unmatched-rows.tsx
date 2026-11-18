import { UnmatchedRow } from "@/features/admin-needs-a-match/components/unmatched-row";
import type { Fixing } from "@/features/admin-needs-a-match/lib/fixing";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";

/** `items`, the Unmatched Items of the Library `libraryId`, each with its Find a Match. */
export function UnmatchedRows({
  items,
  libraryId,
  fixing,
  toggle,
}: { items: UnmatchedItem[]; libraryId: bigint } & Fixing) {
  return (
    // Pulled out by the rows' own padding, so the titles line up with the heading.
    <ul className="-mx-3 mt-2 flex flex-col">
      {items.map((item) => (
        <UnmatchedRow
          key={item.itemId}
          item={item}
          libraryId={libraryId}
          open={fixing === item.itemId}
          onToggle={() => {
            toggle(item.itemId);
          }}
        />
      ))}
    </ul>
  );
}
