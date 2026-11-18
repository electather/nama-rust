import { LibraryFilter } from "@/features/admin-needs-a-match/components/library-filter";
import { UnmatchedLists } from "@/features/admin-needs-a-match/components/unmatched-lists";
import { pickedLibraries } from "@/features/admin-needs-a-match/lib/picked-libraries";
import { Page } from "@/shared/components/page";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * The Needs a Match page: each Library's Unmatched Items, or only the `picked` one's, to Match by
 * hand. `onPick` picks another Library, or every one (`undefined`).
 */
export function NeedsAMatch({
  picked,
  onPick,
}: {
  picked: string | undefined;
  onPick: (library: string | undefined) => void;
}) {
  const libraries = useLibraries();
  const shown = pickedLibraries(libraries, picked);

  return (
    <Page heading="Needs a Match">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Files the Server couldn’t Match to a Movie or Show. Find the right title for each, and it
        stays Matched.
      </p>
      {(libraries.length > 1 || !shown) && (
        <LibraryFilter libraries={libraries} picked={picked} onPick={onPick} />
      )}
      {shown ? (
        <UnmatchedLists libraries={shown} />
      ) : (
        <p className="mt-10 font-medium">That Library isn’t on the Server.</p>
      )}
    </Page>
  );
}
