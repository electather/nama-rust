import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** Every Plugin the Server knows, which the route's loader loads. */
export function usePluginSummaries() {
  return useSuspenseQuery(PluginService.method.listPlugins, {}).data.summaries;
}
