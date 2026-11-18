import { useMutation } from "@connectrpc/connect-query";

import { useRefreshPlugins } from "@/features/admin-plugins/data/use-refresh-plugins";
import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** Enables a Plugin, granting the Plugin Permissions it's given, and refreshes the Plugins once it settles. */
export function useEnablePlugin() {
  return useMutation(PluginService.method.enablePlugin, { onSettled: useRefreshPlugins() });
}
