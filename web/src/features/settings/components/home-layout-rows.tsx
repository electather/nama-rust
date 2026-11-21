import { create } from "@bufbuild/protobuf";

import { ShownRow } from "@/features/settings/components/shown-row";
import { useSetHomeLayout } from "@/features/settings/data/use-set-home-layout";
import { type HomeLayout, type HomeLayoutRow, HomeLayoutSchema } from "@/gen/nama/v1/server_pb";
import { refusal } from "@/shared/lib/forms";
import { rowKey, rowName } from "@/shared/lib/home-layout";

/** Featured and each Home row of the `saved` layout, with a switch that saves it shown or hidden. */
export function HomeLayoutRows({ saved }: { saved: HomeLayout }) {
  const save = useSetHomeLayout();
  // Shows the choice being saved until the layout comes back with it.
  const layout = save.isPending
    ? saving(saved, create(HomeLayoutSchema, save.variables.layout))
    : saved;
  // Names only the flipped row: a row the User leaves alone then keeps following the default,
  // even one the default changed since this list was loaded (`UserService.SetHomeLayout`).
  const flip = (row: HomeLayoutRow, shown: boolean) => {
    save.mutate({
      layout: { featuredHidden: layout.featuredHidden, rows: [{ ...row, hidden: !shown }] },
    });
  };
  return (
    <>
      <ul className="mt-4 flex flex-col">
        <ShownRow
          name={{ label: "Featured", from: "Always at the top" }}
          shown={!layout.featuredHidden}
          onShownChange={(shown) => {
            save.mutate({ layout: { featuredHidden: !shown } });
          }}
        />
        {layout.rows.map((row) => (
          <ShownRow
            key={rowKey(row)}
            name={rowName(row)}
            shown={!row.hidden}
            onShownChange={(shown) => {
              flip(row, shown);
            }}
          />
        ))}
      </ul>
      {save.error && (
        <p role="alert" className="mt-2 text-sm text-destructive">
          {refusal(save.error, "save your Home", {}).form}
        </p>
      )}
    </>
  );
}

/** `saved` with what `asked` changes in it. */
function saving(saved: HomeLayout, asked: HomeLayout): HomeLayout {
  const named = new Map(asked.rows.map((row) => [rowKey(row), row]));
  return {
    ...saved,
    featuredHidden: asked.featuredHidden,
    rows: saved.rows.map((row) => named.get(rowKey(row)) ?? row),
  };
}
