import { useQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The last lines the Plugin `id` logged, with the error if they couldn't load. */
export function usePluginLogLines(id: string) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Plugin Log is an extra under the page, which shows its own error and stays up while it loads
  return useQuery(PluginService.method.listPluginLogLines, { id });
}
