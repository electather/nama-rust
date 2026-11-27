import { type RefObject, useLayoutEffect, useState } from "react";

/** A poster column's narrowest, and the gap between columns (`gap-x-5`), in px. */
const COLUMN = 160;
const GAP = 20;

/** How many columns of at least 160px fit `width`: as CSS's `auto-fill`, but never fewer than 2. */
function columnsIn(width: number) {
  return Math.max(2, Math.floor((width + GAP) / (COLUMN + GAP)));
}

/**
 * How many poster columns the element `ref` holds fits, and how wide each is, following its width.
 * The grid lays them out from this, so the virtualizer's rows are the grid's.
 */
export function useGridColumns(ref: RefObject<HTMLElement | null>) {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const element = ref.current;
    // Without a ResizeObserver, as in tests, the width read once stands.
    const observer =
      typeof ResizeObserver === "function"
        ? new ResizeObserver(([entry]) => {
            setWidth(entry?.contentRect.width ?? 0);
          })
        : undefined;
    if (element) {
      setWidth(element.getBoundingClientRect().width);
      observer?.observe(element);
    }
    return () => {
      observer?.disconnect();
    };
  }, [ref]);
  const columns = columnsIn(width);
  return { columns, columnWidth: Math.max(0, (width - GAP * (columns - 1)) / columns) };
}
