import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute } from "@tanstack/react-router";
import { useId } from "react";

import { Page } from "@/components/page";
import { PluginSettings } from "@/components/plugin-settings";
import { PluginToggle } from "@/components/plugin-toggle";
import { type Plugin, PluginKind, PluginService } from "@/gen/nama/v1/plugin_pb";

export const Route = createFileRoute("/_member/_admin/admin/plugins")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(PluginService.method.listPlugins, {}, { transport }),
      staleTime: "static",
    }),
  component: Plugins,
});

function Plugins() {
  const { plugins } = useSuspenseQuery(PluginService.method.listPlugins, {}).data;
  return (
    <Page heading="Plugins">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Plugins add to what the Server can do, such as finding Movies and Shows on TMDB.
      </p>
      <div className="mt-8 flex max-w-3xl flex-col divide-y">
        {plugins.map((plugin) => (
          <PluginSection key={plugin.id} plugin={plugin} />
        ))}
      </div>
    </Page>
  );
}

function PluginSection({ plugin }: { plugin: Plugin }) {
  const heading = useId();
  return (
    <section aria-labelledby={heading} className="py-6">
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-2">
        <div>
          <h2 id={heading} className="font-medium">
            {plugin.id}
          </h2>
          <p className="text-sm text-muted-foreground">
            {plugin.kind === PluginKind.BUILT_IN ? "Built-in, version " : "Version "}
            {plugin.version}
          </p>
        </div>
        <PluginToggle plugin={plugin} />
      </div>
      {plugin.settings.length > 0 ? (
        <PluginSettings plugin={plugin} />
      ) : (
        <p className="mt-4 text-sm text-muted-foreground">No settings.</p>
      )}
    </section>
  );
}
