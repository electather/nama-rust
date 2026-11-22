import { useQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import type { PluginDetails } from "@/shared/lib/plugins";

/**
 * What `plugin`'s settings check offers on its stored settings, with its error: the choices of its
 * choice settings and its quality profiles. A secret isn't sent, so the Server uses the stored one.
 * Saving the settings refetches it, so what a save made checkable, such as an API key, is offered.
 */
export function useOfferedProfiles(plugin: PluginDetails) {
  const stored = Object.fromEntries(
    plugin.settings.flatMap(({ key, value }) => (value ? [[key, value]] : [])),
  );
  const input = { id: plugin.summary.id, settings: stored };
  const options = { enabled: plugin.checksSettings };
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the check calls the Plugin, which may fail, and the form shows why without the page waiting on it
  return useQuery(PluginService.method.testPluginSettings, input, options);
}
