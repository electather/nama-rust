import type { ReactElement } from "react";

import { usePlugin } from "@/features/admin-plugins/data/use-plugin";
import { type PluginDetails, hasSummary } from "@/shared/lib/plugins";

/**
 * What `children` draws from the Plugin `id`'s details. It suspends until they load, so the parts
 * of a Plugin's view the list already knows needn't wait for them.
 */
export function LoadedPlugin({
  id,
  children,
}: {
  id: string;
  children: (plugin: PluginDetails) => ReactElement;
}): ReactElement | null {
  const plugin = usePlugin(id);
  return hasSummary(plugin) ? children(plugin) : null;
}
