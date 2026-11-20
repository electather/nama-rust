import { focusOnMount } from "@/features/home/lib/focus-on-mount";
import type { Hiding } from "@/features/home/lib/hiding";
import { Button } from "@/shared/ui/button";

/** Says the title `hiding` names is removed, with Undo, which takes the focus. */
export function RemovedNotice({
  hiding,
  onUndo,
}: {
  hiding: Hiding;
  onUndo: (hiding: Hiding) => void;
}) {
  return (
    <span className="mt-2 flex flex-wrap items-center gap-x-2 text-sm text-muted-foreground">
      {hiding.title} is removed from recently watched until you play it again.
      <Button
        variant="ghost"
        ref={focusOnMount}
        onClick={() => {
          onUndo(hiding);
        }}
      >
        Undo
      </Button>
    </span>
  );
}
