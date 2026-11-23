import { Puzzle } from "lucide-react";
import { useRef, useState } from "react";

import { LayoutRow } from "@/shared/components/home-layout/layout-row";
import { RemoveButton } from "@/shared/components/home-layout/remove-button";
import { useRowDrag } from "@/shared/hooks/use-row-drag";
import { useSlideRows } from "@/shared/hooks/use-slide-rows";
import { moved, type RowName } from "@/shared/lib/home-layout";

/**
 * Plugins in the `order` they're asked, called `label`, as Home's rows are shown: each dragged or
 * moved by its arrows, gliding to its new place, or taken out, unless `keepOne` keeps the last.
 * Each change goes to `setOrder`; `nameOf` names each Plugin.
 */
export function OrderList({
  label,
  order,
  setOrder,
  nameOf,
  keepOne = false,
}: {
  label: string;
  order: string[];
  setOrder: (order: string[]) => void;
  nameOf: (key: string) => RowName;
  keepOne?: boolean;
}) {
  const list = useRef<HTMLUListElement>(null);
  const slideRows = useSlideRows(list);
  const [announced, setAnnounced] = useState("");
  const move = (from: number, before: number) => {
    const next = moved(order, from, before);
    const key = order[from];
    if (key !== undefined && next.indexOf(key) !== from) {
      slideRows();
      setOrder(next);
      setAnnounced(
        `${nameOf(key).label} moved to place ${next.indexOf(key) + 1} of ${next.length}.`,
      );
    }
  };
  const rowDrag = useRowDrag(list, move);
  return (
    <div className="-mb-1.5 flex flex-col">
      <ul ref={list} aria-label={label} className="flex flex-col">
        {order.map((key, index) => {
          const name = nameOf(key);
          const remove = (
            <RemoveButton
              label={name.label}
              disabled={keepOne && order.length === 1}
              onRemove={() => {
                setOrder(order.filter((each) => each !== key));
              }}
            />
          );
          return (
            <LayoutRow
              key={key}
              Icon={Puzzle}
              name={name}
              place={{ index, count: order.length }}
              drag={rowDrag(index)}
              onMove={move}
              control={remove}
            />
          );
        })}
      </ul>
      <output aria-live="polite" className="sr-only">
        {announced}
      </output>
    </div>
  );
}
