import { GripVertical } from "lucide-react";
import type { ReactNode } from "react";

import type { HandleProps } from "@/shared/hooks/use-row-drag";

/**
 * The grip a pointer drags a row by, with the row's icon tile, `children`, so the target is wider
 * than the grip alone. Hidden from assistive technology and the keyboard, which move a row with its
 * arrows instead.
 */
export function DragHandle({ children, ...handlers }: HandleProps & { children: ReactNode }) {
  return (
    // Touch scrolling is off on the handle alone, so the pane still scrolls from the rest of the row.
    <span
      aria-hidden
      className="flex shrink-0 cursor-grab touch-none items-center gap-2 py-1.5 select-none group-data-dragging:cursor-grabbing active:cursor-grabbing"
      {...handlers}
    >
      <GripVertical className="size-3.5 text-muted-foreground" />
      <span className="flex transition-opacity group-data-hidden:opacity-45 motion-reduce:transition-none">
        {children}
      </span>
    </span>
  );
}
