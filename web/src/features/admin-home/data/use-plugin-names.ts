import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { loaderStaleTime } from "@/shared/hooks/loader-stale-time";

/** What admins know each Plugin by, by its id: its name, or its id when it gives none. */
export function usePluginNames() {
  const { summaries } = useSuspenseQuery(
    PluginService.method.listPlugins,
    {},
    { staleTime: loaderStaleTime },
  ).data;
  return new Map(summaries.map(({ id, name }) => [id, name === "" ? id : name]));
}
