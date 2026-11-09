import { createConnectQueryKey } from "@connectrpc/connect-query";

import { PluginKind, PluginService } from "@/gen/nama/v1/plugin_pb";

/** The query key of the Plugins list and of each Plugin, to refetch both after a change. */
export const pluginsKey = createConnectQueryKey({ schema: PluginService, cardinality: "finite" });

/** Where a Plugin comes from, and its version if the Server knows it. */
export function kindAndVersion({ kind, version }: { kind: PluginKind; version: string }) {
  const kindName = kind === PluginKind.BUILT_IN ? "Built-in" : "Installed";
  return version === "" ? kindName : `${kindName}, version ${version}`;
}

/** What admins know a Plugin by: its name, or its id when it gives none. */
export function pluginName({ id, name }: { id: string; name: string }) {
  return name === "" ? id : name;
}
