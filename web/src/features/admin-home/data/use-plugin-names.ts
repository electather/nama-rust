import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";
import { pluginName } from "@/shared/lib/plugins";

/** What admins know each Plugin by, by its id. */
export function usePluginNames() {
  const { summaries } = useSuspenseQuery(
    PluginService.method.listPlugins,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  return new Map(summaries.map((plugin) => [plugin.id, pluginName(plugin)]));
}
