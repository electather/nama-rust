import { findUnmatched } from "@/features/admin-needs-a-match/lib/unmatched-group";
import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { LoadedUnmatched } from "@/features/settings/components/loaded-unmatched";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { listSearch } from "@/features/settings/lib/needs-a-match-search";
import { useLibraries } from "@/shared/hooks/use-libraries";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { parsedAs } from "@/shared/lib/unmatched-names";

/**
 * Needs a Match, then the file `?item=` opens, whose Needs a Match crumb goes back to the list
 * still filtered to its Library. A file that isn't listed shows the list, as the pane does.
 */
export function NeedsAMatchCrumbs() {
  const { title } = useOpenSection();
  const { detail } = useSettingsAddress();
  const [first, ...rest] = useLibraries();
  const plain = <Crumbs current={title} />;
  if (detail?.kind !== "unmatched-item" || first === undefined) {
    return plain;
  }
  const { id, inLibrary } = detail;
  return (
    <LoadedUnmatched library={first} rest={rest} before={[]}>
      {(groups) => {
        const found = findUnmatched(groups, id);
        return found === undefined ? (
          plain
        ) : (
          <Crumbs
            parent={<ListCrumb search={listSearch(inLibrary)} />}
            current={parsedAs(found.item)}
          />
        );
      }}
    </LoadedUnmatched>
  );
}
