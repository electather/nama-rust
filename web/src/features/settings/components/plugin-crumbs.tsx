import { Link, useSearch } from "@tanstack/react-router";

import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { Crumbs } from "@/features/settings/components/crumbs";
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
  const back = (
    <Link
      to="."
      search={(rest) => ({ ...rest, plugin: undefined })}
      // The list's search is a subset of the open Plugin's, so inclusive matching marks this current.
      activeOptions={{ exact: true }}
      className="rounded-sm text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {sectionTitles.plugins}
    </Link>
  );
  return <Crumbs parent={back} current={pluginName(open)} />;
}
