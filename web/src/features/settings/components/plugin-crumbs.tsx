import { useSearch } from "@tanstack/react-router";

import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { sectionTitles } from "@/features/settings/lib/sections";
import { pluginName } from "@/shared/lib/plugins";

/**
 * Plugins, then the Plugin `?plugin=` opens, whose Plugins crumb goes back to the list. An id no
 * Plugin has shows the list, as the pane does, so the trail is Plugins alone.
 */
export function PluginCrumbs() {
  const asked = useSearch({ strict: false, select: ({ plugin }) => plugin });
  const open = usePluginSummaries().find((plugin) => plugin.id === asked);
  if (!open) {
    return <Crumbs current={sectionTitles.plugins} />;
  }
  return <Crumbs parent={<ListCrumb section="plugins" />} current={pluginName(open)} />;
}
