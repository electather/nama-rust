import { usePluginSummaries } from "@/features/admin-plugins/data/use-plugin-summaries";
import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { pluginName } from "@/shared/lib/plugins";
import { openedId } from "@/shared/lib/settings-address";

/** Plugins, then the Plugin `?plugin=` opens. An id no Plugin has shows the list, as the pane does. */
export function PluginCrumbs() {
  const { title } = useOpenSection();
  const asked = openedId(useSettingsAddress().detail, "plugin");
  const open = usePluginSummaries().find((plugin) => plugin.id === asked);
  if (!open) {
    return <Crumbs current={title} />;
  }
  return <Crumbs parent={<ListCrumb />} current={pluginName(open)} />;
}
