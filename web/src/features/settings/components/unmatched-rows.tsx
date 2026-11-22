import type { Fixing } from "@/features/admin-needs-a-match/lib/fixing";
import { UnmatchedRow } from "@/features/settings/components/unmatched-row";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";

/** `items`, the Unmatched Items of the Library `libraryId`, each with its Find a Match. */
export function UnmatchedRows({
  items,
  libraryId,
  fixing,
  toggle,
}: { items: UnmatchedItem[]; libraryId: bigint } & Fixing) {
  return (
    <ul className="flex flex-col">
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
