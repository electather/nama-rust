import { useQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The Server's Plugins, once they've loaded, for a User who may list them. */
export function usePlugins() {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- only `Manage settings` lists Plugins, and without the list the Recognisers field shows the stored ids as they are
  return useQuery(PluginService.method.listPlugins, {}).data;
}
