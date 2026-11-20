import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** Every Plugin the Server knows, which the route's loader loads. */
export function usePluginSummaries() {
  return useSuspenseQuery(PluginService.method.listPlugins, {}, { staleTime: loaderStaleTime }).data
    .summaries;
}
