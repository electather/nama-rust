import type { UnmatchedItemSummary } from "@/gen/nama/v1/browse_pb";
import { PlayUnmatched } from "@/shared/components/play-unmatched";
import { parsedAs } from "@/shared/lib/unmatched-names";

/** One Unmatched Item, by what its name says, with the button that plays it. */
export function NotMatchedRow({
  item,
  libraryId,
}: {
  item: UnmatchedItemSummary;
  libraryId: bigint;
}) {
  const name = parsedAs(item);
  return (
    <li className="flex items-center gap-3 border-b py-2 last:border-b-0">
      <span className="min-w-0 flex-1">{name}</span>
      <PlayUnmatched libraryId={libraryId} versionId={item.versionId} name={name} />
    </li>
  );
}
