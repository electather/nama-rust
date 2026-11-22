import { points } from "@/features/admin-plugins/lib/extension-points";
import { ItemRow } from "@/features/settings/components/item-row";
import type { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import { IconTile } from "@/shared/components/icon-tile";

/**
 * The Extension Points a Plugin takes part at, each over a line on what it does there. One from a
 * newer Server than this app knows is left out, and a Plugin left with none has no card.
 */
export function PluginPoints({ extensionPoints }: { extensionPoints: ExtensionPoint[] }) {
  const shown = extensionPoints.flatMap((point) => points[point] ?? []);
  if (shown.length === 0) {
    return null;
  }
  const rows = shown.map(({ label, Icon, means }) => (
    <ItemRow key={label} start={<IconTile Icon={Icon} />} name={label} meta={means} />
  ));
  return (
    <section aria-labelledby="extension-points">
      <h2 id="extension-points" className="font-medium">
        Extension Points
      </h2>
      <ul className="-mb-1.5 flex flex-col">{rows}</ul>
    </section>
  );
}
