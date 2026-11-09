import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import { PowerOff, TriangleAlert } from "lucide-react";

import { Page } from "@/components/page";
import { PluginToggle } from "@/components/plugin-toggle";
import { StateLabel } from "@/components/state-label";
import { buttonVariants } from "@/components/ui/button";
import { type PluginSummary, PluginService } from "@/gen/nama/v1/plugin_pb";
import { kindAndVersion, pluginName } from "@/lib/plugins";

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
      {/* Pulled out by the rows' own padding, so the names line up with the heading. */}
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
      <div className="flex min-w-0 flex-1 basis-64 flex-col items-start">
        <Link
          to="/admin/plugins/$pluginId"
          params={{ pluginId: plugin.id }}
          className="truncate rounded-sm font-medium outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {pluginName(plugin)}
        </Link>
        {plugin.description !== "" && <p className="text-sm text-pretty">{plugin.description}</p>}
        <span className="text-sm text-muted-foreground">{kindAndVersion(plugin)}</span>
        {plugin.enabled && plugin.settingsMissing && <NeedsAttention />}
      </div>
      <EnableOrSetUp plugin={plugin} />
    </li>
  );
}

/** Marks an enabled Plugin missing a required setting, such as one an upgrade added. */
function NeedsAttention() {
  return (
    <div className="mt-1.5">
      <StateLabel label="Needs attention" Icon={TriangleAlert} />
    </div>
  );
}

function EnableOrSetUp({ plugin }: { plugin: PluginSummary }) {
  // Enabling it would only be refused, so it goes to its settings form instead.
  if (!plugin.enabled && plugin.settingsMissing) return <SetUp plugin={plugin} />;
  return <PluginToggle plugin={plugin} />;
}

function SetUp({ plugin }: { plugin: PluginSummary }) {
  return (
    <div className="flex items-center gap-2">
      <StateLabel label="Disabled" Icon={PowerOff} />
      <Link
        to="/admin/plugins/$pluginId"
        params={{ pluginId: plugin.id }}
        aria-label={`Set up ${plugin.id}`}
        className={buttonVariants({ variant: "ghost" })}
      >
        Set up
      </Link>
    </div>
  );
}
