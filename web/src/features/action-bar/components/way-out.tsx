import { ArrowLeft } from "lucide-react";

import type { Bar } from "@/features/action-bar/hooks/use-action-bar";

// Drawn as the key that does the same.
const keyCap =
  "inline-flex h-5 min-w-5 shrink-0 items-center justify-center rounded-sm border bg-muted px-1 text-xs font-medium text-foreground outline-none select-none focus-visible:ring-3 focus-visible:ring-ring/50";

/**
 * The way out of where the action bar is: closing it at its root, stepping back anywhere else. It's
 * the only way out on a touch screen, which has no Escape.
 */
export function WayOut({ bar }: { bar: Bar }) {
  if (bar.frame === "root") {
    return (
      <button type="button" aria-label="Close" onClick={bar.handleClose} className={keyCap}>
        esc
      </button>
    );
  }
  return (
    <button type="button" aria-label="Back" onClick={bar.handleBack} className={keyCap}>
      <ArrowLeft aria-hidden strokeWidth={1.5} className="size-3" />
    </button>
  );
}
