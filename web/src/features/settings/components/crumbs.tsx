import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

/** Where in Settings the pane is: the `current` place, after the `parent` it opened from, if any. */
export function Crumbs({ parent, current }: { parent?: ReactNode; current: string }) {
  const chevron = (
    <ChevronRight
      aria-hidden
      strokeWidth={1.5}
      className="size-4 shrink-0 text-muted-foreground rtl:rotate-180"
    />
  );
  return (
    <ol className="flex min-w-0 items-center gap-1.5 text-sm">
      {parent !== undefined && <li className="flex shrink-0">{parent}</li>}
      <li aria-current="page" className="flex min-w-0 items-center gap-1.5">
        {parent !== undefined && chevron}
        <span className="truncate font-medium">{current}</span>
      </li>
    </ol>
  );
}
