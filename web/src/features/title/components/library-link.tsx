import { Link } from "@tanstack/react-router";

import { useLibraries } from "@/shared/hooks/use-libraries";

/** A link to the Library `libraryId`'s page, by its name, inline in text. */
export function LibraryLink({ libraryId }: { libraryId: bigint }) {
  const library = useLibraries().find(({ id }) => id === libraryId);
  if (!library) {
    return null;
  }
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
