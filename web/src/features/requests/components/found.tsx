import type { ReactNode } from "react";

import { PosterGrid } from "@/shared/components/poster/poster-grid";

/** The `kind` of titles a `search` for `title` found, as they arrive: `children`, one card each. */
export function Found({
  kind,
  title,
  search,
  children,
}: {
  kind: "Movies" | "Shows";
  title: string;
  search: { searching: boolean; failed: boolean };
  children: ReactNode[];
}) {
  const heading = `${kind} for “${title}”`;
  const nothingYet = search.searching ? "Searching…" : `No ${kind} found.`;
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {search.failed && (
        <p role="alert" className="mt-8 text-muted-foreground">
          Couldn’t search for {kind}. Try again in a moment.
        </p>
      )}
      <PosterGrid
        empty={!search.failed && <p className="mt-8 text-muted-foreground">{nothingYet}</p>}
      >
        {children}
      </PosterGrid>
    </section>
  );
}
