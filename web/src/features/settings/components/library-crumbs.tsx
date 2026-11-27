import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { useLibraries } from "@/shared/hooks/use-libraries";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { openedId } from "@/shared/lib/settings-address";

/**
 * Libraries, then New Library or the Library `?library=` opens. An id no Library has shows the
 * list, as the pane does.
 */
export function LibraryCrumbs() {
  const { title } = useOpenSection();
  const asked = openedId(useSettingsAddress().detail, "library");
  const open = useLibraries().find((library) => String(library.id) === asked);
  const name = asked === "new" ? "New Library" : open?.name;
  if (name === undefined) {
    return <Crumbs current={title} />;
  }
  return <Crumbs parent={<ListCrumb />} current={name} />;
}
