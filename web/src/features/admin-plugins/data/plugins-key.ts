import type { Transport } from "@connectrpc/connect";
import { createConnectQueryKey } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The query key of the Plugins list and of each Plugin, to refetch both after a change. */
export function pluginsKey(transport: Transport) {
  return createConnectQueryKey({ schema: PluginService, cardinality: "finite", transport });
}
