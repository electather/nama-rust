import { Link } from "@tanstack/react-router";

import { LibraryCard } from "@/features/home/components/library-card";
import type { Library } from "@/gen/nama/v1/library_pb";
import { posterLink } from "@/shared/lib/poster-classes";

/** A Library's card in a grid, opening the Library. */
export function LibraryLink({ library }: { library: Library }) {
  return (
    <li>
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: String(library.id) }}
        className={posterLink}
      >
        <LibraryCard library={library} />
      </Link>
    </li>
  );
}
