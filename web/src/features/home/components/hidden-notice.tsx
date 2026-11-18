import type { Hiding } from "@/features/home/lib/hiding";
import { Button } from "@/shared/ui/button";

/** A stable ref callback, so React calls it only when the element mounts. */
const focus = (element: HTMLElement | null) => element?.focus();

/**
 * Says the title `lastHidden` is hidden, with Undo, which takes the focus its card's menu lost. It stays
 * until you leave Home or hide another, rather than timing out on someone still reading it.
 */
export function HiddenNotice({
  lastHidden,
  onUndo,
}: {
  lastHidden: Hiding | undefined;
  onUndo: (lastHidden: Hiding) => void;
}) {
  return (
    // Always there, so a screen reader announces what's put in it.
    <output className="block">
      {lastHidden && (
        <span
          key={lastHidden.key}
          className="mt-2 flex flex-wrap items-center gap-x-2 text-sm text-muted-foreground"
        >
          {lastHidden.title} is hidden until you play it again.
          <Button
            variant="ghost"
            ref={focus}
            onClick={() => {
              onUndo(lastHidden);
            }}
          >
            Undo
          </Button>
        </span>
      )}
    </output>
  );
}
