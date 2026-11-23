import { useId } from "react";

import { LibraryCard } from "@/features/home/components/library-card";
import { RowFrame } from "@/features/home/components/row-frame";
import { useLibraries } from "@/shared/hooks/use-libraries";
import { shelfBleed } from "@/shared/lib/shelf-bleed";

/**
 * The Server's Libraries, each by its Backdrop, in a row that scrolls sideways as Home's shelves
 * do, or nothing while it has none.
 */
export function LibraryShelf() {
  const headingId = useId();
  const libraries = useLibraries();
  if (libraries.length === 0) {
    return null;
  }
  return (
    <RowFrame heading="Libraries" headingId={headingId}>
      {/* A Server has a handful of Libraries, so unlike `VirtualShelf` this keeps them all. */}
      <ul
        className={`flex snap-x snap-mandatory scroll-px-shelf-edge scrollbar-none gap-5 overflow-x-auto overscroll-x-contain ${shelfBleed}`}
      >
        {libraries.map((library) => (
          <LibraryCard key={String(library.id)} library={library} />
        ))}
      </ul>
    </RowFrame>
  );
}
