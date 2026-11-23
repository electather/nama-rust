import { Link } from "@tanstack/react-router";

import { LibraryArt } from "@/features/home/components/library-art";
import type { Library } from "@/gen/nama/v1/library_pb";
import { kindName } from "@/shared/lib/library-kind";

/** A Library's item in its shelf: its Backdrop and name, opening the Library, over its kind. */
export function LibraryCard({ library }: { library: Library }) {
  return (
    <li className="w-tile-backdrop shrink-0 snap-start">
      <Link
        to="/libraries/$libraryId"
        params={{ libraryId: String(library.id) }}
        className="flex flex-col gap-1.5 rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <LibraryArt library={library} />
        <span className="text-xs text-muted-foreground">{kindName(library.kind)}</span>
      </Link>
    </li>
  );
}
