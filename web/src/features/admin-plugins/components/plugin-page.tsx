import { notFound } from "@tanstack/react-router";

import { ExtensionPoints } from "@/features/admin-plugins/components/extension-points";
import { LogoChoice } from "@/features/admin-plugins/components/logo-choice";
import { PluginAbout } from "@/features/admin-plugins/components/plugin-about";
import { PluginIdentity } from "@/features/admin-plugins/components/plugin-identity";
import { PluginLog } from "@/features/admin-plugins/components/plugin-log";
import { PluginQualities } from "@/features/admin-plugins/components/plugin-qualities";
import { PluginSettingsSection } from "@/features/admin-plugins/components/plugin-settings-section";
import { PluginToggle } from "@/features/admin-plugins/components/plugin-toggle";
import { PointFailures } from "@/features/admin-plugins/components/point-failures";
import { usePlugin } from "@/features/admin-plugins/data/use-plugin";
import { Page } from "@/shared/components/page";
import { hasSummary, pluginName } from "@/shared/lib/plugins";

/** The Plugin `id`'s page: what it is, its state, and a section per part of its setup. */
// oxlint-disable-next-line import/max-dependencies -- the page puts together a section per part of a Plugin's setup: settings, Qualities, log
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
      {summary.refusedReason === "" ? (
        <>
          <PluginAbout plugin={plugin} />
          <LogoChoice summary={summary} />
          <PluginSettingsSection plugin={plugin} />
          <PluginQualities plugin={plugin} />
          <PluginLog id={summary.id} />
        </>
      ) : (
        <p className="mt-6 max-w-prose text-sm text-pretty text-muted-foreground">
          The Server can’t run it: {summary.refusedReason}.
        </p>
      )}
    </Page>
  );
}
