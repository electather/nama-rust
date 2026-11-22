import { useSearch } from "@tanstack/react-router";

import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { PluginDetail } from "@/features/settings/components/plugin-detail";
import { PluginList } from "@/features/settings/components/plugin-list";

/**
 * Every Plugin on the Server, for a User who manages settings: the list of them, or in its place
 * the one `?plugin=<id>` names, to enable, disable or set up.
 */
export function PluginsPane() {
  const summaries = usePluginSummaries();
  const asked = useSearch({ strict: false, select: ({ plugin }) => plugin });
  // An id no Plugin has shows the list, as an address from before one was uninstalled would.
  const open = summaries.find((plugin) => plugin.id === asked);
  return open ? <PluginDetail summary={open} /> : <PluginList summaries={summaries} />;
}
