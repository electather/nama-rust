import { type RefObject, useLayoutEffect, useRef } from "react";

import { listRows } from "@/shared/lib/list-rows";

/**
 * Slides the rows of `list` from where they are when the returned function is called to where the
 * next render puts them, so a row moved, or dropped, glides into its place rather than jumping.
 * Rows match across the render by their element, which React keeps by key.
 */
export function useSlideRows(list: RefObject<HTMLElement | null>) {
  const tops = useRef<Map<Element, number>>(undefined);
  useLayoutEffect(() => {
    const was = tops.current;
    tops.current = undefined;
    if (was !== undefined && !globalThis.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      slideAll(listRows(list.current), was);
    }
  });
  return () => {
    tops.current = new Map(listRows(list.current).map((row) => [row, top(row)]));
  };
}

function top(row: Element) {
  return row.getBoundingClientRect().top;
}

/** Slides each of `rows` from its top in `was` to where it is now. */
function slideAll(rows: Element[], was: Map<Element, number>) {
  for (const row of rows) {
    const by = (was.get(row) ?? top(row)) - top(row);
    if (by !== 0) {
      // DESIGN.md's ease-out-expo, as `--ease-out-expo` isn't readable here.
      row.animate([{ translate: `0 ${by}px` }, { translate: "0 0" }], {
        duration: 250,
        easing: "cubic-bezier(0.16, 1, 0.3, 1)",
      });
    }
  }
}
