import {
  type PluginOrderKind,
  labelOf,
  recogniserNote,
} from "@/features/admin-libraries/lib/recognisers";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { OrderList } from "@/shared/components/home-layout/order-list";

/**
 * `order` as Home's rows are shown, each with its Plugin among `recognisers`, once the Plugins are
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
    <OrderList
      label={kind.legend}
      order={order}
      setOrder={setOrder}
      nameOf={(pluginId) => {
        const plugin = recognisers.find((each) => each.id === pluginId);
        return { label: labelOf(pluginId, plugin), from: recogniserNote(plugin, listed) };
      }}
    />
  );
}
