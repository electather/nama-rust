import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/shared/lib/plugins";

/**
 * Local disk and `stores` to pick from. The store `current`, when it isn't one of them (a disabled
 * or uninstalled Plugin's), comes between them: chosen, but not pickable.
 */
export function StoreOptions({
  current,
  stores,
  nameOf,
}: {
  current: string;
  stores: PluginSummary[];
  nameOf: (pluginId: string) => string;
}) {
  const inUseButDisabled = current !== "" && !stores.some((plugin) => plugin.id === current);
  return (
    <>
      <option value="">Local disk</option>
      {inUseButDisabled && (
        <option value={current} disabled>
          {nameOf(current)} (disabled)
        </option>
      )}
      {stores.map((plugin) => (
        <option key={plugin.id} value={plugin.id}>
          {pluginName(plugin)}
        </option>
      ))}
    </>
  );
}
