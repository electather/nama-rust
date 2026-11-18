import { type ReactNode, useId } from "react";

import { FilesToRename } from "@/features/admin-needs-a-match/components/files-to-rename";
import type { Library } from "@/gen/nama/v1/library_pb";

/** `library`'s Unmatched Items (`children`), then the files whose names say nothing. */
export function LibrarySection({
  library,
  unparsedPaths,
  children,
}: {
  library: Library;
  unparsedPaths: string[];
  children: ReactNode;
}) {
  const heading = useId();
  return (
    <section aria-labelledby={heading} className="mt-10 max-w-3xl">
      <h2 id={heading} className="font-medium">
        {library.name}
      </h2>
      {children}
      {unparsedPaths.length > 0 && <FilesToRename paths={unparsedPaths} />}
    </section>
  );
}
