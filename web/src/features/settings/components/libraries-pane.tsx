import { useSearch } from "@tanstack/react-router";

import { LibraryDetail } from "@/features/settings/components/library-detail";
import { LibraryList } from "@/features/settings/components/library-list";
import { NewLibrary } from "@/features/settings/components/new-library";
import { useLibraries } from "@/shared/hooks/use-libraries";

/**
 * The Server's Libraries, for a User who manages them: the list of them, or in its place the form
 * `?library=new` opens, or the one `?library=<id>` names, to scan and change.
 */
export function LibrariesPane() {
  const libraries = useLibraries();
  const asked = useSearch({ strict: false, select: ({ library }) => library });
  // An id no Library has shows the list, as an address from before one was deleted would.
  const open = libraries.find((library) => String(library.id) === asked);
  if (asked === "new") {
    return <NewLibrary />;
  }
  return open ? (
    <LibraryDetail key={open.id} listed={open} />
  ) : (
    <LibraryList libraries={libraries} />
  );
}
