import { type PointerEvent, type RefObject, useState } from "react";

/** A row being dragged by the pointer `pointerId`, from `from`, to drop before the row at `before`. */
interface Drag {
  pointerId: number;
  from: number;
  before: number;
}

/** What one row gets of a drag: its handle's props, the edge a drop marks, and whether it's dragged. */
export interface RowDrag {
  handle: ReturnType<ReturnType<typeof useRowDrag>["handleProps"]>;
  marker: "top" | "bottom" | undefined;
  dragging: boolean;
}

/**
 * Reordering a list of `count` rows by dragging each by its handle, which `drop` gets on release:
 * what the row at each index gets of it.
 */
export function useRowDrag(
  list: RefObject<HTMLUListElement | null>,
  count: number,
  drop: (from: number, before: number) => void,
) {
  const [drag, setDrag] = useState<Drag>();
  const isOurs = (event: PointerEvent) => drag?.pointerId === event.pointerId;

  const handleProps = (index: number) => ({
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      if (event.button !== 0 || drag !== undefined) {
        return;
      }
      // Captured, so the drag goes on once the pointer leaves the handle.
      event.currentTarget.setPointerCapture(event.pointerId);
      setDrag({ pointerId: event.pointerId, from: index, before: index });
    },
    onPointerMove: (event: PointerEvent) => {
      if (drag !== undefined && isOurs(event)) {
        setDrag({ ...drag, before: dropSpot(list.current, event.clientY) });
      }
    },
    onPointerUp: (event: PointerEvent) => {
      if (drag !== undefined && isOurs(event)) {
        setDrag(undefined);
        drop(drag.from, drag.before);
      }
    },
    onLostPointerCapture: () => {
      setDrag(undefined);
    },
  });

  const before = dropBefore(drag);
  const rowDrag = (index: number): RowDrag => ({
    handle: handleProps(index),
    marker: markerAt(index, before, count),
    dragging: drag?.from === index,
  });
  return { handleProps, rowDrag };
}

/** Where `drag` would drop, or `undefined` just before or after its own row, which stays put. */
function dropBefore(drag: Drag | undefined) {
  const staysPut = drag === undefined || drag.before === drag.from || drag.before === drag.from + 1;
  return staysPut ? undefined : drag.before;
}

/** Which edge of the row at `index`, of `count`, marks a drop `before`, if either. */
function markerAt(index: number, before: number | undefined, count: number) {
  if (before === index) {
    return "top";
  }
  return before === count && index === count - 1 ? "bottom" : undefined;
}

/**
 * Which row of `list` a pointer at `y` drops before: the first whose middle is below it, by
 * layout, or the row count to drop last.
 */
function dropSpot(list: HTMLUListElement | null, y: number) {
  const rows = [...(list?.children ?? [])];
  const spot = rows.findIndex((row) => {
    const { top, height } = row.getBoundingClientRect();
    return y < top + height / 2;
  });
  return spot === -1 ? rows.length : spot;
}
