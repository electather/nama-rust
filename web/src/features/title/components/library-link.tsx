import { Link } from "@tanstack/react-router";

import type { Library } from "@/gen/nama/v1/library_pb";

/** A link to `library`'s page, by its name, inline in text. */
export function LibraryLink({ library }: { library: Library }) {
  return (
    <Link
      to="/libraries/$libraryId"
      params={{ libraryId: String(library.id) }}
      className="rounded-sm text-foreground underline underline-offset-3 outline-none focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {library.name}
    </Link>
  );
}
