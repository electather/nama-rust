import { ChevronRight } from "lucide-react";
import { type ReactNode, useEffect, useRef } from "react";

import { useCrumbsRoot } from "@/features/settings/hooks/crumbs-root";

/**
 * Where in Settings the pane is: the `current` place, after the `parent` it opened from, if any,
 * and on a phone after the crumb back to the list of sections (`CrumbsRoot`).
 * When the parent goes, as going back to a list does, focus lost with it moves to `current`.
 */
export function Crumbs({ parent, current }: { parent?: ReactNode; current: string }) {
  const root = useCrumbsRoot();
  const rooted = root !== undefined;
  const currentCrumb = useRef<HTMLLIElement>(null);
  const opened = parent !== undefined;
  const wasOpened = useRef(opened);
  useEffect(() => {
    // The parent crumb or the opened thing held focus, and both go on the way back. Focus that
    // didn't go with them, as on the section's own link, stays.
    if (wasOpened.current && !opened && focusLost()) {
      currentCrumb.current?.focus();
    }
    wasOpened.current = opened;
  }, [opened]);
  const chevron = (
    <ChevronRight
      aria-hidden
      strokeWidth={1.5}
      className="size-4 shrink-0 text-muted-foreground rtl:rotate-180"
    />
  );
  return (
    <ol className="flex min-w-0 items-center gap-1.5 text-sm">
      {rooted && <li className="flex shrink-0">{root}</li>}
      {opened && (
        <li className="flex shrink-0 items-center gap-1.5">
          {rooted && chevron}
          {parent}
        </li>
      )}
      <li
        ref={currentCrumb}
        aria-current="page"
        tabIndex={-1}
        className="flex min-w-0 items-center gap-1.5 outline-none"
      >
        {(opened || rooted) && chevron}
        <span className="truncate font-medium">{current}</span>
      </li>
    </ol>
  );
}

// Focus falls to the body when what held it unmounts, but may stay on it while a Suspense fallback
// hides it, until it unmounts too.
function focusLost() {
  const focused = document.activeElement;
  return focused === null || focused === document.body || !focused.checkVisibility();
}
