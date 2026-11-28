import { usePrefetchUnmatchedItems } from "@/features/admin-needs-a-match/data/use-unmatched-items";
import { LoadedUnmatched } from "@/features/settings/components/loaded-unmatched";
import { NeedsAMatch } from "@/features/settings/components/needs-a-match";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * The files each Library couldn't Match by itself, for a User who manages Libraries, to Match by
 * hand: the list of them, or in its place the one `?item=<id>` opens.
 */
export function NeedsAMatchPane() {
  const libraries = useLibraries();
  usePrefetchUnmatchedItems(libraries);
  const [first, ...rest] = libraries;
  if (first === undefined) {
    return <NeedsAMatch groups={[]} />;
  }
  return (
    <LoadedUnmatched library={first} rest={rest} before={[]}>
      {(groups) => <NeedsAMatch groups={groups} />}
    </LoadedUnmatched>
  );
}
