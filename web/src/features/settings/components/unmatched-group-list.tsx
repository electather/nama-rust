import { useId } from "react";

import { FilesToRename } from "@/features/admin-needs-a-match/components/files-to-rename";
import type { UnmatchedGroup } from "@/features/admin-needs-a-match/lib/unmatched-group";
import { UnmatchedLink } from "@/features/settings/components/unmatched-link";
import { ShowMore } from "@/shared/components/show-more";

/**
 * `group`'s Library, if it has files that need a Match: its name over them in one inset list, a
 * Show more for the next page, and the files that only a rename can fix. `picked` is the Library
 * filtered to, if one is, which each file's screen remembers.
 */
export function UnmatchedGroupList({
  group,
  picked,
}: {
  group: UnmatchedGroup;
  picked: string | undefined;
}) {
  const nameId = useId();
  const { library, items, unparsedPaths, hasNextPage, loadedPages, loadMore } = group;
  if (group.count === 0) {
    return null;
  }
  return (
    <section aria-labelledby={nameId} className="flex flex-col gap-2">
      <h4 id={nameId} className="px-3.5 text-xs font-medium text-muted-foreground">
        {library.name}
      </h4>
      {items.length > 0 && (
        <ul className="overflow-hidden rounded-xl bg-card">
          {items.map((item) => (
            <UnmatchedLink key={item.itemId} item={item} picked={picked} />
          ))}
        </ul>
      )}
      {hasNextPage && <ShowMore load={loadMore} loaded={loadedPages} />}
      <FilesToRename paths={unparsedPaths} />
    </section>
  );
}
