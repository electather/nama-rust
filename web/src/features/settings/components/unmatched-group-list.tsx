import { FilesToRename } from "@/features/admin-needs-a-match/components/files-to-rename";
import type { UnmatchedGroup } from "@/features/admin-needs-a-match/lib/unmatched-group";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UnmatchedLink } from "@/features/settings/components/unmatched-link";
import { ShowMore } from "@/shared/components/show-more";

/**
 * `group`'s Library, if it has files that need a Match: its name over them, a Show more for the
 * next page, and the files that only a rename can fix. `picked` is the Library filtered to, if one
 * is, which each file's screen remembers.
 */
export function UnmatchedGroupList({
  group,
  picked,
}: {
  group: UnmatchedGroup;
  picked: string | undefined;
}) {
  const { library, items, unparsedPaths, hasNextPage, loadedPages, loadMore } = group;
  if (group.count === 0) {
    return null;
  }
  return (
    <SettingGroup title={library.name}>
      <ul aria-label={library.name} className="-mb-1.5 flex flex-col empty:hidden">
        {items.map((item) => (
          <UnmatchedLink key={item.itemId} item={item} picked={picked} />
        ))}
      </ul>
      {hasNextPage && <ShowMore load={loadMore} loaded={loadedPages} />}
      <FilesToRename paths={unparsedPaths} />
    </SettingGroup>
  );
}
