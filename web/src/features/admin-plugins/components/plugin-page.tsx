import { notFound } from "@tanstack/react-router";

import { ExtensionPoints } from "@/features/admin-plugins/components/extension-points";
import { PluginIdentity } from "@/features/admin-plugins/components/plugin-identity";
import { PluginSections } from "@/features/admin-plugins/components/plugin-sections";
import { PluginToggle } from "@/features/admin-plugins/components/plugin-toggle";
import { PointFailures } from "@/features/admin-plugins/components/point-failures";
import { usePlugin } from "@/features/admin-plugins/data/use-plugin";
import { Page } from "@/shared/components/page";
import { hasSummary, pluginName } from "@/shared/lib/plugins";

/** The Plugin `id`'s page: what it is, its state, and a section per part of its setup. */
export function PluginPage({ id }: { id: string }) {
  const plugin = usePlugin(id);
  if (!hasSummary(plugin)) {
    throw notFound();
  }
  const { summary } = plugin;
  return (
    <Page heading={pluginName(summary)}>
      <PluginIdentity summary={summary} />
      {summary.description !== "" && (
        <p className="mt-3 max-w-prose text-pretty">{summary.description}</p>
      )}
      <ExtensionPoints extensionPoints={summary.extensionPoints} />
      <div className="mt-4 flex flex-wrap items-center gap-x-3 gap-y-2">
        <PluginToggle summary={summary} settings={plugin.settings} />
      </div>
      <PointFailures failures={plugin.failures} />
      <PluginSections plugin={plugin} />
    </Page>
  );
}
