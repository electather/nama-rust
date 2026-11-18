import type { ReactNode } from "react";

import { PosterGrid } from "@/shared/components/poster/poster-grid";

/** The `kind` of titles a `search` for `title` found, once it's done: `children`, one card each. */
export function Found({
  kind,
  title,
  search,
  children,
}: {
  kind: "Movies" | "Shows";
  title: string;
  search: { isPending: boolean; isError: boolean };
  children: ReactNode[] | undefined;
}) {
  const heading = `${kind} for “${title}”`;
  return (
    <section aria-label={heading} className="mt-10">
      <h2 className="font-medium">{heading}</h2>
      {search.isPending && <p className="mt-8 text-muted-foreground">Searching…</p>}
      {search.isError && (
        <p role="alert" className="mt-8 text-muted-foreground">
          Couldn’t search for {kind}. Try again in a moment.
        </p>
      )}
      {children && (
        <PosterGrid empty={<p className="mt-8 text-muted-foreground">No {kind} found.</p>}>
          {children}
        </PosterGrid>
      )}
    </section>
  );
}
