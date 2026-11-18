import { createConnectQueryKey } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The query key of every Plugin call, to refetch them once a logo changes. */
export const pluginQueriesKey = createConnectQueryKey({
  schema: PluginService,
  cardinality: undefined,
});
