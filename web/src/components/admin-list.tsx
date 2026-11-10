import type { ReactNode } from "react";

/** The list on an admin page, such as its Plugins, Libraries or Users. */
export function AdminList({ children }: { children: ReactNode }) {
  return (
    // Pulled out by the rows' own padding, so what starts each row lines up with the heading.
    <ul className="mt-8 -mx-3 flex max-w-2xl flex-col">{children}</ul>
  );
}

/** One row of an `AdminList`, between an optional `start` and an `end` that wraps below when narrow. */
export function AdminRow({
  start,
  end,
  children,
}: {
  start?: ReactNode;
  end: ReactNode;
  children: ReactNode;
}) {
  return (
    <li className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-3 py-3 last:border-b-0">
      {start}
      {/* Capped, as `items-start` lets a child grow past the column, so a `truncate` name overflows. */}
      <div className="flex min-w-0 flex-1 basis-64 flex-col items-start *:max-w-full">
        {children}
      </div>
      {end}
    </li>
  );
}
