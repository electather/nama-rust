import { useSearch } from "@tanstack/react-router";

import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { sectionTitles } from "@/features/settings/lib/sections";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * Libraries, then New Library or the Library `?library=` opens. An id no Library has shows the
 * list, as the pane does.
 */
export function LibraryCrumbs() {
  const asked = useSearch({ strict: false, select: ({ library }) => library });
  const open = useLibraries().find((library) => String(library.id) === asked);
  const name = asked === "new" ? "New Library" : open?.name;
  if (name === undefined) {
    return <Crumbs current={sectionTitles.libraries} />;
  }
  return <Crumbs parent={<ListCrumb section="libraries" />} current={name} />;
}
