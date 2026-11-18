import { PluginToggle } from "@/features/admin-plugins/components/plugin-toggle";
import { SetUp } from "@/features/admin-plugins/components/set-up";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** A Plugin's toggle, or for one missing a required setting a link to set it up first. */
export function EnableOrSetUp({ plugin }: { plugin: PluginSummary }) {
  // Enabling it would only be refused, so it goes to its settings form instead.
  if (!plugin.enabled && plugin.settingsMissing) {
    return <SetUp plugin={plugin} />;
  }
  return <PluginToggle summary={plugin} />;
}
