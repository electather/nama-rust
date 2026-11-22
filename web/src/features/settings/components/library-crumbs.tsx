import { useSearch } from "@tanstack/react-router";

import { SectionCrumbs } from "@/features/settings/components/section-crumbs";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * Libraries, then New Library or the Library `?library=` opens. An id no Library has shows the
 * list, as the pane does.
 */
export function LibraryCrumbs() {
  const asked = useSearch({ strict: false, select: ({ library }) => library });
  const open = useLibraries().find((library) => String(library.id) === asked);
  return <SectionCrumbs section="libraries" open={asked === "new" ? "New Library" : open?.name} />;
}
