import { focusOnMount } from "@/features/home/lib/focus-on-mount";
import type { Marking } from "@/features/home/lib/marking";

/** Says the title `marking` names is marked as watched, taking the focus. */
export function WatchedNotice({ marking }: { marking: Marking }) {
  return (
    <span
      ref={focusOnMount}
      tabIndex={-1}
      className="mt-2 block text-sm text-muted-foreground outline-none"
    >
      {marking.title} is marked as watched.
    </span>
  );
}
