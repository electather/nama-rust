import { Link } from "@tanstack/react-router";

import { reasonCaption } from "@/features/admin-needs-a-match/lib/unmatched-reason";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { detailSearch } from "@/shared/lib/settings-address";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * What a file's name was parsed as, a link stretched over its row that opens the file's own
 * screen, over why it needs a Match. `picked` is the Library the list is filtered to, if one is.
 */
export function UnmatchedName({
  item,
  picked,
}: {
  item: UnmatchedItem;
  picked: string | undefined;
}) {
  return (
    <div className="flex min-w-0 flex-1 flex-col gap-0.5">
      <Link
        to="."
        search={(rest) => ({
          ...rest,
          ...detailSearch({ kind: "unmatched-item", id: String(item.itemId), inLibrary: picked }),
        })}
        className="truncate text-base font-medium outline-none after:absolute after:inset-0 focus-visible:after:ring-3 focus-visible:after:ring-ring/50 focus-visible:after:ring-inset"
      >
        {parsedAs(item)}
      </Link>
      <span className="text-xs text-pretty text-muted-foreground">{reasonCaption(item)}</span>
    </div>
  );
}
