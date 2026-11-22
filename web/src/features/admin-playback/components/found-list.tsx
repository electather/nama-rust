import type { ReactNode } from "react";

/** The programs or hardware the Server found, listed under the words that introduce them. */
export function FoundList({ children }: { children: ReactNode }) {
  return (
    // Pulled out by the rows' own padding, so their words line up with the text above.
    <ul className="-mx-3 mt-4 flex flex-col">{children}</ul>
  );
}
