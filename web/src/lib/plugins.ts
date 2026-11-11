import { createConnectQueryKey } from "@connectrpc/connect-query";

import {
  type Plugin,
  PluginKind,
  PluginService,
  type PluginSummary,
} from "@/gen/nama/v1/plugin_pb";

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

/** A Plugin with its summary, which every Plugin the Server sends holds. */
export type PluginDetails = Plugin & { summary: PluginSummary };

/** Whether `plugin` holds its summary, so its page can show it. */
export function hasSummary(plugin: Plugin | undefined): plugin is PluginDetails {
  return plugin?.summary !== undefined;
}
