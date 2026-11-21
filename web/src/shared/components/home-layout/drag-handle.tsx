import { GripVertical } from "lucide-react";

import type { HandleProps } from "@/shared/hooks/use-row-drag";

/**
 * The grip a pointer drags a row by. Hidden from assistive technology and the keyboard, which move
 * a row with its arrows instead.
 */
export function DragHandle(handlers: HandleProps) {
  return (
    // Touch scrolling is off on the grip alone, so the page still scrolls from the rest of the row.
    <span
      aria-hidden
      className="flex h-10 w-6 shrink-0 cursor-grab touch-none items-center justify-center text-muted-foreground select-none active:cursor-grabbing"
      {...handlers}
    >
      <GripVertical className="size-4" />
    </span>
  );
}
