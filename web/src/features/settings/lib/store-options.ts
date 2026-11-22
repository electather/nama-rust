import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/shared/lib/plugins";
import type { SelectOption } from "@/shared/lib/select-option";

/**
 * Local disk and `stores` to pick from. The store `current`, when it isn't one of them (a disabled
 * or uninstalled Plugin's), comes between them: chosen, but not pickable.
 */
export function storeOptions(
  current: string,
  stores: PluginSummary[],
  nameOf: (pluginId: string) => string,
): SelectOption[] {
  const inUseButDisabled = current !== "" && !stores.some((plugin) => plugin.id === current);
  return [
    { value: "", label: "Local disk" },
    ...(inUseButDisabled
      ? [{ value: current, label: `${nameOf(current)} (disabled)`, disabled: true }]
      : []),
    ...stores.map((plugin) => ({ value: plugin.id, label: pluginName(plugin) })),
  ];
}
