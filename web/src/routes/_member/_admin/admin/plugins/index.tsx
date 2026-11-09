import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";

import { Page } from "@/components/page";
import { PluginToggle } from "@/components/plugin-toggle";
import { type PluginSummary, PluginService } from "@/gen/nama/v1/plugin_pb";
import { kindAndVersion } from "@/lib/plugins";

export const Route = createFileRoute("/_member/_admin/admin/plugins/")({
  loader: ({ context: { queryClient, transport } }) =>
    queryClient.query({
      ...createQueryOptions(PluginService.method.listPlugins, {}, { transport }),
      staleTime: "static",
    }),
  component: Plugins,
});

function Plugins() {
  const { summaries } = useSuspenseQuery(PluginService.method.listPlugins, {}).data;
  return (
    <Page heading="Plugins">
      <p className="mt-2 max-w-prose text-pretty text-muted-foreground">
        Plugins add to what the Server can do, such as finding Movies and Shows on TMDB.
      </p>
      {/* Pulled out by the rows' own padding, so the ids line up with the heading. */}
      <ul className="mt-8 -mx-3 flex max-w-2xl flex-col">
        {summaries.map((plugin) => (
          <PluginRow key={plugin.id} plugin={plugin} />
        ))}
      </ul>
    </Page>
  );
}

function PluginRow({ plugin }: { plugin: PluginSummary }) {
  return (
    <li className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b px-3 py-3 last:border-b-0">
      <div className="me-auto flex min-w-0 flex-col">
        <Link
          to="/admin/plugins/$pluginId"
          params={{ pluginId: plugin.id }}
          className="truncate rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {plugin.id}
        </Link>
        <span className="text-sm text-muted-foreground">{kindAndVersion(plugin)}</span>
      </div>
      <PluginToggle plugin={plugin} />
    </li>
  );
}
