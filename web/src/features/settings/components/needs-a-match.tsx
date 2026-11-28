import {
  type UnmatchedGroup,
  findUnmatched,
} from "@/features/admin-needs-a-match/lib/unmatched-group";
import { NeedsAMatchDetail } from "@/features/settings/components/needs-a-match-detail";
import { NeedsAMatchList } from "@/features/settings/components/needs-a-match-list";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { openedId } from "@/shared/lib/settings-address";

/**
 * Needs a Match once its Items have loaded: the file `?item=` opens, pushed over the list, or the
 * list, filtered to the Library `?library=` names. An Item that isn't listed, such as one just
 * Matched, shows the list.
 */
export function NeedsAMatch({ groups }: { groups: UnmatchedGroup[] }) {
  const { detail } = useSettingsAddress();
  if (detail?.kind === "unmatched-item") {
    const { id, inLibrary } = detail;
    const found = findUnmatched(groups, id);
    if (found !== undefined) {
      return <NeedsAMatchDetail {...found} inLibrary={inLibrary} />;
    }
    return <NeedsAMatchList groups={groups} picked={inLibrary} />;
  }
  return <NeedsAMatchList groups={groups} picked={openedId(detail, "library-filter")} />;
}
