import { useRef, useState } from "react";

import { FeaturedRow } from "@/features/admin-home/components/featured-row";
import { LayoutRow } from "@/features/admin-home/components/layout-row";
import { useDefaultHomeLayout } from "@/features/admin-home/data/use-default-home-layout";
import { usePluginNames } from "@/features/admin-home/data/use-plugin-names";
import { useSetDefaultHomeLayout } from "@/features/admin-home/data/use-set-default-home-layout";
import { useLayoutRows } from "@/features/admin-home/hooks/use-layout-rows";
import { useRowDrag } from "@/features/admin-home/hooks/use-row-drag";
import { saveRefused } from "@/features/admin-home/lib/home-layout";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { rowKey, rowName } from "@/shared/lib/home-layout";

/** The default Home layout as an Admin changes it, saved as a whole. */
export function HomeLayoutForm() {
  const saved = useDefaultHomeLayout();
  const names = usePluginNames();
  const save = useSetDefaultHomeLayout();
  const [featuredHidden, setFeaturedHidden] = useState(saved.featuredHidden);
  const { rows, move, setShown, announced } = useLayoutRows(saved.rows, names);
  const list = useRef<HTMLUListElement>(null);
  const rowDrag = useRowDrag(list, rows.length, move);

  return (
    <form
      className="mt-8 flex max-w-2xl flex-col"
      onSubmit={(event) => {
        event.preventDefault();
        save.mutate({ layout: { featuredHidden, rows } });
      }}
    >
      <FeaturedRow
        shown={!featuredHidden}
        onShownChange={(shown) => {
          setFeaturedHidden(!shown);
        }}
      />
      <ul ref={list} className="-mx-3 flex flex-col">
        {rows.map((row, index) => (
          <LayoutRow
            key={rowKey(row)}
            name={rowName(row, names)}
            shown={!row.hidden}
            place={{ index, count: rows.length }}
            drag={rowDrag(index)}
            onMove={move}
            onShownChange={(shown) => {
              setShown(index, shown);
            }}
          />
        ))}
      </ul>
      <output aria-live="polite" className="sr-only">
        {announced}
      </output>
      <div className="mt-6 flex flex-col gap-5">
        <FormEnd
          action={save.isPending ? "Saving Home layout…" : "Save Home layout"}
          pending={save.isPending}
          done={save.isSuccess ? "Saved." : undefined}
          refusal={saveRefused(save.error)}
        />
      </div>
    </form>
  );
}
