import type { ReactNode } from "react";

/** The list in an admin section of Settings, such as its Plugins, Libraries or Users. */
export function AdminList({ children }: { children: ReactNode }) {
  return (
    // Pulled out by the rows' own padding, so what starts each row lines up with the heading.
    <ul className="-mx-3 mt-8 flex max-w-2xl flex-col">{children}</ul>
  );
}
