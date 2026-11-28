import { useNavigate } from "@tanstack/react-router";
import { useId } from "react";

import type { UnmatchedGroup } from "@/features/admin-needs-a-match/lib/unmatched-group";
import { UnmatchedHeader } from "@/features/settings/components/unmatched-header";
import { listSearch } from "@/features/settings/lib/needs-a-match-search";
import type { UnmatchedItem } from "@/gen/nama/v1/match_pb";
import { FindAMatch } from "@/shared/components/find-a-match/find-a-match";
import { notInShow } from "@/shared/lib/find-a-match";

/**
 * One file of `group`'s Library on its own screen, in place of the list, whose crumbs name it: why it needs a
 * Match, Find a Match for it, and Match at the foot of the pane. Matching goes back to the list,
 * which no longer has it, unless its Show lacks its Episode, which Matching says in place. `inLibrary`
 * is the Library the list was filtered to.
 */
export function NeedsAMatchDetail({
  group,
  item,
  inLibrary,
}: {
  group: UnmatchedGroup;
  item: UnmatchedItem;
  inLibrary: string | undefined;
}) {
  const titleId = useId();
  const navigate = useNavigate();
  return (
    <section
      aria-labelledby={titleId}
      className="flex min-h-full flex-col bg-background px-4 pt-4 md:px-5 md:pt-2"
    >
      <FindAMatch
        key={item.itemId}
        item={item}
        libraryId={group.library.id}
        above={<UnmatchedHeader titleId={titleId} item={item} libraryId={group.library.id} />}
        onMatched={(_, matched) => {
          if (notInShow(item, matched) === "") {
            void navigate({ to: ".", search: (rest) => ({ ...rest, ...listSearch(inLibrary) }) });
          }
        }}
      />
    </section>
  );
}
