import { createConnectQueryKey } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The query key of the Plugins list and of each Plugin, to refetch both after a change. */
export const pluginsKey = createConnectQueryKey({ schema: PluginService, cardinality: "finite" });
