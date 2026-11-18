import type { ReactNode } from "react";

import { posterGrid } from "@/shared/lib/poster-classes";

/** A grid of posters, or what to say instead when there are none. */
export function PosterGrid({
  empty,
  children,
}: {
  empty: ReactNode;
  children: ReactNode[];
}): ReactNode {
  if (children.length === 0) {
    return empty;
  }
  return <ul className={posterGrid}>{children}</ul>;
}
