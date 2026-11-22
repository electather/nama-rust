import { type PointerEvent, type RefObject, useState } from "react";

import { listRows } from "@/shared/lib/list-rows";

/**
 * A row being dragged by the pointer `pointerId`, from `from`, to drop before the row at `before`:
 * how far it's been dragged, how tall it is, and each row's middle as the drag began, before any
 * row slid out of its way.
 */
interface Drag {
  pointerId: number;
  from: number;
  before: number;
  startY: number;
  dy: number;
  height: number;
  middles: number[];
}

/** What the handle a row is dragged by listens to. */
export interface HandleProps {
  onPointerDown: (event: PointerEvent<HTMLElement>) => void;
  onPointerMove: (event: PointerEvent) => void;
  onPointerUp: (event: PointerEvent) => void;
  onLostPointerCapture: () => void;
}

/**
 * What one row gets of a drag: its handle's props, how far it's moved from its place, as the row
 * dragged or one sliding out of its way, and whether it's the one dragged or one that slides.
 */
export interface RowDrag {
  handle: HandleProps;
  offset: number;
  dragging: boolean;
  sliding: boolean;
}

/**
 * Reordering a list of rows by dragging each by its handle, which `drop` gets on release: what the
 * row at each index gets of it. The row dragged follows the pointer, and the rows it passes slide
 * out of its way to open where it would drop.
 */
export function useRowDrag(
  list: RefObject<HTMLUListElement | null>,
  drop: (from: number, before: number) => void,
) {
  const [drag, setDrag] = useState<Drag>();
  const isOurs = (event: PointerEvent) => drag?.pointerId === event.pointerId;

  const handleProps = (index: number): HandleProps => ({
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      if (event.button !== 0 || drag !== undefined) {
        return;
      }
      // Captured, so the drag goes on once the pointer leaves the handle.
      event.currentTarget.setPointerCapture(event.pointerId);
      const rects = listRows(list.current).map((row) => row.getBoundingClientRect());
      setDrag(startDrag(rects, index, event));
    },
    onPointerMove: (event: PointerEvent) => {
      if (drag !== undefined && isOurs(event)) {
        const before = dropSpot(drag.middles, event.clientY);
        setDrag({ ...drag, before, dy: event.clientY - drag.startY });
      }
    },
    onPointerUp: (event: PointerEvent) => {
      if (drag !== undefined && isOurs(event)) {
        drop(drag.from, drag.before);
        setDrag(undefined);
      }
    },
    onLostPointerCapture: () => {
      setDrag(undefined);
    },
  });

  const rowDrag = (index: number): RowDrag => ({
    handle: handleProps(index),
    offset: offsetOf(drag, index),
    dragging: drag?.from === index,
    sliding: drag !== undefined && drag.from !== index,
  });
  return rowDrag;
}

/** A drag of the row at `index`, of the rows whose bounds are `rects`, by the pointer `event` pressed. */
function startDrag(rects: DOMRect[], index: number, { pointerId, clientY }: PointerEvent): Drag {
  return {
    pointerId,
    from: index,
    before: index,
    startY: clientY,
    dy: 0,
    height: rects[index]?.height ?? 0,
    middles: rects.map(({ top, height }) => top + height / 2),
  };
}

/**
 * How far the row at `index` is from its place during `drag`: the row dragged, by as far as the
 * pointer went; a row it passed, by its height, the other way.
 */
function offsetOf(drag: Drag | undefined, index: number) {
  if (drag === undefined) {
    return 0;
  }
  if (index === drag.from) {
    return drag.dy;
  }
  const passedUp = within(drag.before, index, drag.from);
  const passedDown = within(drag.from + 1, index, drag.before);
  return drag.height * (Number(passedUp) - Number(passedDown));
}

function within(start: number, index: number, end: number) {
  return start <= index && index < end;
}

/**
 * Which row a pointer at `y` drops before: the first whose middle, as the drag began, is below it,
 * or the row count to drop last.
 */
function dropSpot(middles: number[], y: number) {
  const spot = middles.findIndex((middle) => y < middle);
  return spot === -1 ? middles.length : spot;
}
