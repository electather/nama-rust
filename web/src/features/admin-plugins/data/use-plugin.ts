import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The Plugin `id`, which the route's loader loads. */
export function usePlugin(id: string) {
  return useSuspenseQuery(PluginService.method.getPlugin, { id }).data.plugin;
}
