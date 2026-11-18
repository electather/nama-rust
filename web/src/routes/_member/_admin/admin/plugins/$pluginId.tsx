import { createFileRoute } from "@tanstack/react-router";

import { NotFound } from "@/app/route-error/not-found";
import { PluginPage } from "@/features/admin-plugins/components/plugin-page";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { queryOrNotFound } from "@/shared/hooks/browse";

export const Route = createFileRoute("/_member/_admin/admin/plugins/$pluginId")({
  loader: ({ context, params }) =>
    queryOrNotFound(context, PluginService.method.getPlugin, { id: params.pluginId }),
  notFoundComponent: NotFound,
  component: PluginRoute,
});

function PluginRoute() {
  const { pluginId } = Route.useParams();
  return <PluginPage id={pluginId} />;
}
