import { useSearch } from "@tanstack/react-router";

import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { SectionCrumbs } from "@/features/settings/components/section-crumbs";
import { pluginName } from "@/shared/lib/plugins";

/** Plugins, then the Plugin `?plugin=` opens. An id no Plugin has shows the list, as the pane does. */
export function PluginCrumbs() {
  const asked = useSearch({ strict: false, select: ({ plugin }) => plugin });
  const open = usePluginSummaries().find((plugin) => plugin.id === asked);
  return <SectionCrumbs section="plugins" open={open && pluginName(open)} />;
}
