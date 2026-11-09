import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { PluginSettings } from "@/components/plugin-settings";
import { PluginToggle } from "@/components/plugin-toggle";
import { NotFound } from "@/components/route-error";
import { type Plugin, PluginService } from "@/gen/nama/v1/plugin_pb";
import { queryOrNotFound } from "@/lib/browse";
import { kindAndVersion } from "@/lib/plugins";

export const Route = createFileRoute("/_member/_admin/admin/plugins/$pluginId")({
  loader: ({ context, params }) =>
    queryOrNotFound(context, PluginService.method.getPlugin, { id: params.pluginId }),
  notFoundComponent: NotFound,
  component: PluginPage,
});

function PluginPage() {
  const { pluginId } = Route.useParams();
  const { plugin } = useSuspenseQuery(PluginService.method.getPlugin, { id: pluginId }).data;
  if (!plugin) throw notFound();
  return (
    <Page heading={plugin.id}>
      <p className="mt-1 text-sm text-muted-foreground">{kindAndVersion(plugin)}</p>
      <div className="mt-4 flex flex-wrap items-center gap-x-3 gap-y-2">
        <PluginToggle plugin={plugin} />
      </div>
      {plugin.refusedReason === "" ? (
        <Settings plugin={plugin} />
      ) : (
        <p className="mt-6 max-w-prose text-sm text-pretty text-muted-foreground">
          The Server can’t run it: {plugin.refusedReason}.
        </p>
      )}
    </Page>
  );
}

function Settings({ plugin }: { plugin: Plugin }) {
  return (
    <section aria-labelledby="settings" className="mt-10">
      <h2 id="settings" className="font-medium">
        Settings
      </h2>
      {plugin.settings.length > 0 ? (
        <PluginSettings plugin={plugin} />
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">No settings.</p>
      )}
    </section>
  );
}
