import { useNavigate } from "@tanstack/react-router";
import { useId } from "react";

import { EverythingMatched } from "@/features/admin-needs-a-match/components/everything-matched";
import {
  type UnmatchedGroup,
  filesNeedingAMatch,
} from "@/features/admin-needs-a-match/lib/unmatched-group";
import { LibraryChoice } from "@/features/settings/components/library-choice";
import { NeedsAMatchHeading } from "@/features/settings/components/needs-a-match-heading";
import { UnmatchedGroupList } from "@/features/settings/components/unmatched-group-list";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { listSearch } from "@/features/settings/lib/needs-a-match-search";

/**
 * The list of what needs a Match: a large heading with how many files there are, the Library they
 * are filtered to (`picked`, an id) and each Library's files in a list of its own, each opening
 * its own screen.
 */
export function NeedsAMatchList({
  groups,
  picked,
}: {
  groups: UnmatchedGroup[];
  picked: string | undefined;
}) {
  const titleId = useId();
  const { title } = useOpenSection();
  const navigate = useNavigate();
  const total = filesNeedingAMatch(groups);
  const shown =
    picked === undefined ? groups : groups.filter(({ library }) => String(library.id) === picked);
  const missing = picked !== undefined && shown.length === 0;
  const filtered = (
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
  );
  return (
    <section
      aria-labelledby={titleId}
      className="flex flex-col gap-7 p-4 md:px-7.5 md:pt-5 md:pb-8"
    >
      <NeedsAMatchHeading titleId={titleId} title={title} total={total}>
        {filtered}
      </NeedsAMatchHeading>
      {shown.map((group) => (
        <UnmatchedGroupList key={group.library.id} group={group} picked={picked} />
      ))}
      {missing ? (
        <p className="text-sm text-muted-foreground">That Library isn’t on the Server.</p>
      ) : (
        <EverythingMatched anyListed={shown.some((group) => group.count > 0)} />
      )}
    </section>
  );
}
