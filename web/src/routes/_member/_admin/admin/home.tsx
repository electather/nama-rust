import { createQueryOptions } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";

import { HomeLayoutPage } from "@/features/admin-home/components/home-layout-page";
import { defaultHomeLayoutQuery } from "@/features/admin-home/data/use-default-home-layout";
import { PluginService } from "@/gen/nama/v1/plugin_pb";

export const Route = createFileRoute("/_member/_admin/admin/home")({
  loader: ({ context: { queryClient, transport } }) =>
    Promise.all([
      queryClient.query(defaultHomeLayoutQuery(transport)),
      // The rows from Plugins are named by their Plugin.
      queryClient.query({
        ...createQueryOptions(PluginService.method.listPlugins, {}, { transport }),
        staleTime: "static",
      }),
    ]),
  component: HomeLayoutPage,
});
