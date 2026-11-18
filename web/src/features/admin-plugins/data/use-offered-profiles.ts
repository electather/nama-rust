import { useQuery } from "@connectrpc/connect-query";

import { PluginService, type SettingValue } from "@/gen/nama/v1/plugin_pb";

/** What the settings check of the Plugin `id` offers on its `stored` settings, with its error. */
export function useOfferedProfiles(id: string, stored: Record<string, SettingValue>) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the check calls the Plugin, which may fail, and the form shows why without the page waiting on it
  return useQuery(PluginService.method.testPluginSettings, { id, settings: stored });
}
