import { RecogniserRow } from "@/features/admin-libraries/components/recognisers-field/recogniser-row";
import { type PluginOrderKind, moves } from "@/features/admin-libraries/lib/recognisers";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/**
 * `order` as a list of rows, each with its Plugin among `recognisers`, once the Plugins are
 * `listed`. Nothing while it's empty.
 */
export function RecogniserList({
  kind,
  order,
  setOrder,
  recognisers,
  listed,
}: {
  kind: PluginOrderKind;
  order: string[];
  setOrder: (order: string[]) => void;
  recognisers: PluginSummary[];
  listed: boolean;
}) {
  if (order.length === 0) {
    return null;
  }
  return (
    <ol aria-label={kind.legend} className="flex flex-col gap-1">
      {order.map((pluginId, index) => (
        <RecogniserRow
          key={pluginId}
          name={kind.name}
          pluginId={pluginId}
          plugin={recognisers.find((each) => each.id === pluginId)}
          listed={listed}
          moves={moves(order, index, setOrder)}
        />
      ))}
    </ol>
  );
}
