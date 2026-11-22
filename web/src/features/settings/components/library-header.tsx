import { useEffect, useRef } from "react";

import type { Library } from "@/gen/nama/v1/library_pb";
import { kindName } from "@/shared/lib/library-kind";

/** The top of a Library's view, over a hairline: its name over what it holds. */
export function LibraryHeader({ titleId, library }: { titleId: string; library: Library }) {
  const title = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    // The list this replaced held focus, so it moves to what took its place.
    title.current?.focus();
  }, []);
  return (
    <header className="px-4 pt-4 md:px-6 md:pt-2">
      <div className="flex flex-col gap-0.5 border-b pb-6">
        <h3
          ref={title}
          id={titleId}
          tabIndex={-1}
          className="truncate text-lg font-medium outline-none"
        >
          {library.name}
        </h3>
        <p className="text-sm text-muted-foreground">{kindName(library.kind)}</p>
      </div>
    </header>
  );
}
