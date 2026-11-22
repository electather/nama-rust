import { useNavigate, useSearch } from "@tanstack/react-router";

import { LibraryFilter } from "@/features/admin-needs-a-match/components/library-filter";
import { pickedLibraries } from "@/features/admin-needs-a-match/lib/picked-libraries";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { UnmatchedLists } from "@/features/settings/components/unmatched-lists";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * Each Library's Unmatched Items, or only those of the one `?library=<id>` picks, to Match by hand,
 * for a User who manages Libraries.
 */
export function NeedsAMatchPane() {
  const libraries = useLibraries();
  const picked = useSearch({ strict: false, select: ({ library }) => library });
  const navigate = useNavigate();
  const shown = pickedLibraries(libraries, picked);
  const filter = (libraries.length > 1 || !shown) && (
    <LibraryFilter
      libraries={libraries}
      picked={picked}
      onPick={(library) => {
        // As a section's link does, so Back still closes Settings.
        void navigate({ to: ".", search: (rest) => ({ ...rest, library }), replace: true });
      }}
    />
  );
  return (
    <>
      {filter}
      <PaneFrame section="needs-a-match">
        <p className="text-sm text-pretty text-muted-foreground">
          Files the Server couldn’t Match to a Movie or Show. Find the right title for each, and it
          stays Matched.
        </p>
        {shown ? (
          <UnmatchedLists libraries={shown} />
        ) : (
          <p className="text-sm text-muted-foreground">That Library isn’t on the Server.</p>
        )}
      </PaneFrame>
    </>
  );
}
