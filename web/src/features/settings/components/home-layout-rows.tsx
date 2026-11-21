import { useRef } from "react";

import { useHomeLayoutChanges } from "@/features/settings/hooks/use-home-layout-changes";
import type { HomeLayout } from "@/gen/nama/v1/server_pb";
import { FeaturedRow } from "@/shared/components/home-layout/featured-row";
import { LayoutRow } from "@/shared/components/home-layout/layout-row";
import { useRowDrag } from "@/shared/hooks/use-row-drag";
import { refusal } from "@/shared/lib/forms";
import { rowKey, rowName } from "@/shared/lib/home-layout";

/**
 * Featured and each Home row of the `saved` layout, with a switch that saves it shown or hidden,
 * and each row dragged or moved by its arrows into the User's own order.
 */
export function HomeLayoutRows({ saved }: { saved: HomeLayout }) {
  const { layout, setFeaturedShown, setShown, move, announced, error } =
    useHomeLayoutChanges(saved);
  const list = useRef<HTMLUListElement>(null);
  const rowDrag = useRowDrag(list, layout.rows.length, move);
  return (
    <div className="mt-4 flex flex-col">
      <FeaturedRow shown={layout.featuredHidden !== true} onShownChange={setFeaturedShown} />
      <ul ref={list} className="-mx-3 flex flex-col">
        {layout.rows.map((row, index) => (
          <LayoutRow
            key={rowKey(row)}
            name={rowName(row)}
            shown={!row.hidden}
            place={{ index, count: layout.rows.length }}
            drag={rowDrag(index)}
            onMove={move}
            onShownChange={(shown) => {
              setShown(row, shown);
            }}
          />
        ))}
      </ul>
      <output aria-live="polite" className="sr-only">
        {announced}
      </output>
      {error && (
        <p role="alert" className="mt-2 text-sm text-destructive">
          {refusal(error, "save your Home", {}).form}
        </p>
      )}
    </div>
  );
}
