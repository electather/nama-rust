import { type VirtualItem, useWindowVirtualizer } from "@tanstack/react-virtual";
import {
  type ReactNode,
  type RefObject,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { posterGrid } from "@/components/poster";

declare module "react" {
  interface CSSProperties {
    /** `VirtualPosterGrid`'s padding standing in for the rows above those it shows. */
    "--rows-above"?: string;
    /** As `--rows-above`, for the rows below. */
    "--rows-below"?: string;
  }
}

/** `posterGrid`'s breakpoints, widest first, as Tailwind's `lg`, `md` and `sm` place them. */
const wider = ["(min-width: 64rem)", "(min-width: 48rem)", "(min-width: 40rem)"];

function subscribe(changed: () => void) {
  const queries = wider.map((query) => matchMedia(query));
  for (const query of queries) {
    query.addEventListener("change", changed);
  }
  return () => {
    for (const query of queries) {
      query.removeEventListener("change", changed);
    }
  };
}

/** How many columns `posterGrid` has at the window's width now: 5, 4, 3 or 2. */
function posterColumns() {
  const widest = wider.findIndex((query) => matchMedia(query).matches);
  return widest === -1 ? 2 : 5 - widest;
}

/** Where the element `ref` holds starts, down the page, once it is laid out. */
function useTopOf(ref: RefObject<HTMLElement | null>) {
  const [top, setTop] = useState(0);
  useLayoutEffect(() => {
    setTop((ref.current?.getBoundingClientRect().top ?? 0) + window.scrollY);
  }, [ref]);
  return top;
}

/**
 * The padding that stands in for the rows above and below `rows`, so the grid keeps its height
 * and columns: rows start below `top` and the last ends at `end`, both down the page.
 */
function paddingFor(rows: VirtualItem[], end: number, top: number) {
  const [start = top] = rows.map((row) => row.start);
  const shownEnd = rows.reduce((_, row) => row.end, top);
  return { above: `${start - top}px`, below: `${end - shownEnd}px` };
}

/** A poster's row's height at `lg`, with its caption, before it is measured. */
const ROW_ESTIMATE = 360;
/** `posterGrid`'s `gap-y-8`, between rows. */
const ROW_GAP = 32;

/**
 * A grid of `posters` that keeps only the rows near the screen in the page, however many there
 * are, so a long Library scrolls with a bounded number of DOM nodes. `setSize` is how many the
 * whole list has, for assistive technology.
 */
export function VirtualPosterGrid({
  posters,
  setSize,
}: {
  posters: { key: string; poster: ReactNode }[];
  setSize: number | undefined;
}) {
  const columns = useSyncExternalStore(subscribe, posterColumns, () => 5);
  const list = useRef<HTMLUListElement>(null);
  const top = useTopOf(list);
  // oxlint-disable-next-line react/incompatible-library -- the virtualizer re-renders this component itself whenever the window scrolls
  const virtualizer = useWindowVirtualizer({
    count: Math.ceil(posters.length / columns),
    estimateSize: () => ROW_ESTIMATE,
    gap: ROW_GAP,
    overscan: 2,
    scrollMargin: top,
  });
  const rows = virtualizer.getVirtualItems();
  const padding = paddingFor(rows, virtualizer.getTotalSize() + top, top);
  return (
    <ul
      ref={list}
      className={`${posterGrid} pt-(--rows-above) pb-(--rows-below)`}
      style={{ "--rows-above": padding.above, "--rows-below": padding.below }}
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
