import { createQueryOptions, useSuspenseQuery } from "@connectrpc/connect-query";
import { Link, createFileRoute } from "@tanstack/react-router";
import { PowerOff, TriangleAlert } from "lucide-react";

import { AdminList, AdminRow } from "@/components/admin-list";
import { ExtensionPoints, PluginLogo } from "@/components/plugin-at-a-glance";
import { Page } from "@/components/page";
import { PluginToggle } from "@/components/plugin-toggle";
import { StateLabel } from "@/components/state-label";
import { buttonVariants } from "@/components/ui/button";
import { type PluginSummary, PluginService } from "@/gen/nama/v1/plugin_pb";
// oxlint-disable-next-line import/max-dependencies -- each row puts together a Plugin’s logo, what it is, its state and how to enable or set it up
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
      <AdminList>
        {summaries.map((plugin) => (
          <PluginRow key={plugin.id} plugin={plugin} />
        ))}
      </AdminList>
    </Page>
  );
}

function PluginRow({ plugin }: { plugin: PluginSummary }) {
  return (
    <AdminRow start={<PluginLogo plugin={plugin} />} end={<EnableOrSetUp plugin={plugin} />}>
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
      <ExtensionPoints extensionPoints={plugin.extensionPoints} />
    </AdminRow>
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
