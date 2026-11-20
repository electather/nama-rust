import { create } from "@bufbuild/protobuf";

import { ShownRow } from "@/features/settings/components/shown-row";
import { useSetHomeLayout } from "@/features/settings/data/use-set-home-layout";
import { type HomeLayout, HomeLayoutSchema } from "@/gen/nama/v1/server_pb";
import { refusal } from "@/shared/lib/forms";
import { rowKey, rowName } from "@/shared/lib/home-layout";

/** Featured and each Home row of the `saved` layout, with a switch that saves it shown or hidden. */
export function HomeLayoutRows({ saved }: { saved: HomeLayout }) {
  const save = useSetHomeLayout();
  // Shows the choice being saved until the layout comes back with it.
  const layout = save.isPending ? create(HomeLayoutSchema, save.variables.layout) : saved;
  return (
    <>
      <ul className="mt-4 flex flex-col">
        <ShownRow
          name={{ label: "Featured", from: "Always at the top" }}
          shown={!layout.featuredHidden}
          onShownChange={(shown) => {
            save.mutate({ layout: { ...layout, featuredHidden: !shown } });
          }}
        />
        {layout.rows.map((row, index) => (
          <ShownRow
            key={rowKey(row)}
            name={rowName(row)}
            shown={!row.hidden}
            onShownChange={(shown) => {
              const rows = layout.rows.map((each, at) =>
                at === index ? { ...each, hidden: !shown } : each,
              );
              save.mutate({ layout: { ...layout, rows } });
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
