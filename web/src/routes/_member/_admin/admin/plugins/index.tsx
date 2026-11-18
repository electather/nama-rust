import { createQueryOptions } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { Plugins } from "@/features/admin-plugins/components/plugins";
import { PluginService } from "@/gen/nama/v1/plugin_pb";

export const Route = createFileRoute("/_member/_admin/admin/plugins/")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(PluginService.method.listPlugins, {}, { transport }),
      staleTime: "static",
    }),
  component: Plugins,
});
