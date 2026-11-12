import { useSuspenseQuery } from "@connectrpc/connect-query";
import { createFileRoute, notFound } from "@tanstack/react-router";

import { ExtensionPoints, LogoChoice, PluginLogo } from "@/components/plugin-at-a-glance";
import { Page } from "@/components/page";
import { PluginQualities } from "@/components/plugin-qualities";
import { PluginSettings } from "@/components/plugin-settings";
import { PluginToggle } from "@/components/plugin-toggle";
import { NotFound } from "@/components/route-error";
import { PluginService } from "@/gen/nama/v1/plugin_pb";
import { queryOrNotFound } from "@/lib/browse";
// oxlint-disable-next-line import/max-dependencies -- the page puts together a section per part of a Plugin's setup: settings, Qualities
import { hasSummary, kindAndVersion, type PluginDetails, pluginName } from "@/lib/plugins";

export const Route = createFileRoute("/_member/_admin/admin/plugins/$pluginId")({
  loader: ({ context, params }) =>
    queryOrNotFound(context, PluginService.method.getPlugin, { id: params.pluginId }),
  notFoundComponent: NotFound,
  component: PluginPage,
});

function PluginPage() {
  const { pluginId } = Route.useParams();
  const { plugin } = useSuspenseQuery(PluginService.method.getPlugin, { id: pluginId }).data;
  if (!hasSummary(plugin)) throw notFound();
  const { summary } = plugin;
  return (
    <Page heading={pluginName(summary)}>
      <div className="mt-3 flex items-center gap-3">
        <PluginLogo plugin={summary} />
        <p className="text-sm text-muted-foreground">
          <span>{summary.id}</span> · <span>{kindAndVersion(summary)}</span>
        </p>
      </div>
      {summary.description !== "" && (
        <p className="mt-3 max-w-prose text-pretty">{summary.description}</p>
      )}
      <ExtensionPoints extensionPoints={summary.extensionPoints} />
      <div className="mt-4 flex flex-wrap items-center gap-x-3 gap-y-2">
        <PluginToggle summary={summary} settings={plugin.settings} />
      </div>
      {summary.refusedReason === "" ? (
        <>
          <LogoChoice summary={summary} />
          <Settings plugin={plugin} />
          <PluginQualities plugin={plugin} />
        </>
      ) : (
        <p className="mt-6 max-w-prose text-sm text-pretty text-muted-foreground">
          The Server can’t run it: {summary.refusedReason}.
        </p>
      )}
    </Page>
  );
}

function Settings({ plugin }: { plugin: PluginDetails }) {
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
