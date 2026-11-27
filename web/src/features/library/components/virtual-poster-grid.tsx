import { useWindowVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useEffect, useRef } from "react";

import { useGridColumns } from "@/features/library/hooks/use-grid-columns";
import { useTopOf } from "@/features/library/hooks/use-top-of";
import type { Jump } from "@/features/library/lib/titles-props";
import { paddingFor } from "@/shared/lib/padding-for";

declare module "react" {
  interface CSSProperties {
    /** `VirtualPosterGrid`'s padding standing in for the rows above those it shows. */
    "--rows-above"?: string;
    /** As `--rows-above`, for the rows below. */
    "--rows-below"?: string;
    /** `VirtualPosterGrid`'s columns, as many as `useGridColumns` fits. */
    "--columns"?: string;
  }
}

/** The grid's `gap-y-7.5`, between rows. */
const ROW_GAP = 30;
/** Where a row scrolled to rests: under the top bar, 72px (`h-18`), and 24px more, as the rail. */
const UNDER_TOP_BAR = 96;

/**
 * A grid of `posters` that keeps only the rows near the screen in the page, however many there
 * are, so a long Library scrolls with a bounded number of DOM nodes. `setSize` is how many the
 * whole list has, for assistive technology. Once `jump`'s target poster is in `posters`, it
 * scrolls its row to the top and moves focus to it.
 */
export function VirtualPosterGrid({
  posters,
  setSize,
  jump,
}: {
  posters: { key: string; poster: ReactNode }[];
  setSize: number | undefined;
  jump?: Jump | undefined;
}) {
  const list = useRef<HTMLUListElement>(null);
  const { columns, columnWidth } = useGridColumns(list);
  const top = useTopOf(list);
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders this component itself whenever the window scrolls
  const virtualizer = useWindowVirtualizer({
    count: Math.ceil(posters.length / columns),
    // A 2:3 poster, until the row is measured.
    estimateSize: () => columnWidth * 1.5,
    gap: ROW_GAP,
    overscan: 2,
    scrollMargin: top,
    scrollPaddingStart: UNDER_TOP_BAR,
  });
  const focusing = useRef<number>(undefined);
  const target = jump?.target;
  useEffect(() => {
    if (target === undefined || target >= posters.length) {
      return;
    }
    virtualizer.scrollToIndex(Math.floor(target / columns), { align: "start" });
    // As `aria-posinset` counts it, from 1.
    focusing.current = target + 1;
    jump?.reached();
  }, [target, posters.length, columns, virtualizer, jump]);
  // The target's row is in the page only once the scroll has rendered it.
  useEffect(() => {
    const at = focusing.current;
    const poster = list.current?.querySelector<HTMLElement>(`[aria-posinset="${String(at)}"] a`);
    if (at !== undefined && poster) {
      focusing.current = undefined;
      poster.focus({ preventScroll: true });
    }
  });
  const rows = virtualizer.getVirtualItems();
  const padding = paddingFor(rows, virtualizer.getTotalSize() + top, top);
  return (
    <ul
      ref={list}
      className="grid grid-cols-(--columns) gap-x-5 gap-y-7.5 pt-(--rows-above) pb-(--rows-below)"
      style={{
        "--columns": `repeat(${String(columns)}, minmax(0, 1fr))`,
        "--rows-above": padding.above,
        "--rows-below": padding.below,
      }}
    >
      {rows.flatMap((row) =>
        posters
          .slice(row.index * columns, (row.index + 1) * columns)
          .map(({ key, poster }, column) => (
            <li
              key={key}
              // Each row's first poster is measured for the row: the grid stretches it to the row.
              ref={column === 0 ? virtualizer.measureElement : undefined}
              data-index={row.index}
              aria-setsize={setSize}
              aria-posinset={row.index * columns + column + 1}
            >
              {poster}
            </li>
          )),
      )}
    </ul>
  );
}
