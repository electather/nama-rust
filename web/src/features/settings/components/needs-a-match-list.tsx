import { useNavigate } from "@tanstack/react-router";

import { EverythingMatched } from "@/features/admin-needs-a-match/components/everything-matched";
import {
  type UnmatchedGroup,
  filesNeedingAMatch,
} from "@/features/admin-needs-a-match/lib/unmatched-group";
import { LibraryChoice } from "@/features/settings/components/library-choice";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { UnmatchedGroupList } from "@/features/settings/components/unmatched-group-list";
import { listSearch } from "@/features/settings/lib/needs-a-match-search";

/**
 * The list of what needs a Match: how many files there are, the Library they are filtered to
 * (`picked`, an id) and each Library's files in a group of its own, each opening its own screen.
 */
export function NeedsAMatchList({
  groups,
  picked,
}: {
  groups: UnmatchedGroup[];
  picked: string | undefined;
}) {
  const navigate = useNavigate();
  const total = filesNeedingAMatch(groups);
  const shown =
    picked === undefined ? groups : groups.filter(({ library }) => String(library.id) === picked);
  const missing = picked !== undefined && shown.length === 0;
  return (
    <PaneFrame>
      <p className="text-xs text-pretty text-muted-foreground empty:hidden">
        {total > 0 &&
          `The Server couldn’t tell what ${total === 1 ? "this file is" : `these ${total} files are`}. Open one to pick its title.`}
      </p>
      <LibraryChoice
        groups={groups}
        picked={picked}
        missing={missing}
        onPick={(library) => {
          void navigate({
            to: ".",
            search: (rest) => ({ ...rest, ...listSearch(library) }),
            replace: true,
          });
        }}
      />
      {shown.map((group) => (
        <UnmatchedGroupList key={group.library.id} group={group} picked={picked} />
      ))}
      {missing ? (
        <p className="text-sm text-muted-foreground">That Library isn’t on the Server.</p>
      ) : (
        <EverythingMatched anyListed={shown.some((group) => group.count > 0)} />
      )}
    </PaneFrame>
  );
}
