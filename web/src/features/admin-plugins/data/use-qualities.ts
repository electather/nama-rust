import { useQuery } from "@connectrpc/connect-query";

import { PluginService } from "@/gen/nama/v1/plugin_pb";

/** The Qualities of the Plugin `id`, with the error if they couldn't load. */
export function useQualities(id: string) {
  // fallow-ignore-next-line policy-violation:feature-layout/suspense -- the Qualities are a section of the page, which shows its own error and stays up while it loads
  return useQuery(PluginService.method.listQualities, { id });
}
