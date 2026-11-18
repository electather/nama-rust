import { useSuspenseQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/**
 * Every Plugin, enabled or not. Suspends while the list loads, so the picker mounts with the store in
 * use among its options: an uncontrolled select applies its `defaultValue` only at mount.
 */
export function usePlugins() {
  return useSuspenseQuery(PluginService.method.listPlugins, {}).data.summaries;
}
