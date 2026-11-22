import { CirclePlay, Puzzle } from "lucide-react";
import { useRef } from "react";

import type { HomeLayout, HomeLayoutRow } from "@/gen/nama/v1/server_pb";
import { FeaturedRow } from "@/shared/components/home-layout/featured-row";
import { LayoutRow } from "@/shared/components/home-layout/layout-row";
import { useRowDrag } from "@/shared/hooks/use-row-drag";
import { useSlideRows } from "@/shared/hooks/use-slide-rows";
import { rowKey, rowName, rowSetting } from "@/shared/lib/home-layout";

/** A Home layout as it's shown while it changes, and how its list asks for each change. */
export interface LayoutChanges {
  layout: HomeLayout;
  setFeaturedShown: (shown: boolean) => void;
  setShown: (row: HomeLayoutRow, shown: boolean) => void;
  move: (from: number, before: number) => void;
  /** Where the last row moved went, for assistive technology to announce. */
  announced: string;
}

/**
 * Featured and each Home row of a layout, each with a switch to show or hide it, and each row
 * dragged or moved by its arrows, gliding to its new place. `names` names the Plugins rows come
 * from, and `refused` says why the Server refused a change.
 */
export function LayoutList({
  changes: { layout, setFeaturedShown, setShown, move, announced },
  names,
  refused,
}: {
  changes: LayoutChanges;
  names?: Map<string, string>;
  refused: string | undefined;
}) {
  const list = useRef<HTMLUListElement>(null);
  const slideRows = useSlideRows(list);
  const slidingMove = (from: number, before: number) => {
    slideRows();
    move(from, before);
  };
  const rowDrag = useRowDrag(list, slidingMove);
  return (
    <div className="-mb-1.5 flex flex-col">
      <FeaturedRow shown={layout.featuredHidden !== true} onShownChange={setFeaturedShown} />
      <ul ref={list} className="flex flex-col">
        {layout.rows.map((row, index) => (
          <LayoutRow
            key={rowKey(row)}
            setting={rowSetting(row)}
            Icon={row.row.case === "kernel" ? CirclePlay : Puzzle}
            name={rowName(row, names)}
            shown={!row.hidden}
            place={{ index, count: layout.rows.length }}
            drag={rowDrag(index)}
            onMove={slidingMove}
            onShownChange={(shown) => {
              setShown(row, shown);
            }}
          />
        ))}
      </ul>
      <output aria-live="polite" className="sr-only">
        {announced}
      </output>
      {refused !== undefined && (
        <p role="alert" className="mt-2 text-xs text-destructive">
          {refused}
        </p>
      )}
    </div>
  );
}
