import { LogoChoice } from "@/features/admin-plugins/components/logo-choice";
import { PluginAbout } from "@/features/admin-plugins/components/plugin-about";
import { PluginJobs } from "@/features/admin-plugins/components/plugin-jobs";
import { PluginLog } from "@/features/admin-plugins/components/plugin-log";
import { PluginQualities } from "@/features/admin-plugins/components/plugin-qualities";
import { PluginSettingsSection } from "@/features/admin-plugins/components/plugin-settings-section";
import type { PluginDetails } from "@/shared/lib/plugins";

/**
 * A section per part of a Plugin's setup: about, logo, settings, Qualities, Jobs and log. One the
 * Server refused has none of them, only why it can't run. A `headed` view gives the Plugin's links
 * and changes its logo in its own header, so About leaves the links out and there's no logo section.
 */
export function PluginSections({
  plugin,
  headed = false,
}: {
  plugin: PluginDetails;
  headed?: boolean;
}) {
  const { summary } = plugin;
  if (summary.refusedReason !== "") {
    return (
      <p className="mt-6 max-w-prose text-sm text-pretty text-muted-foreground">
        The Server can’t run it: {summary.refusedReason}.
      </p>
    );
  }
  return (
    <>
      <PluginAbout plugin={plugin} links={!headed} />
      {!headed && <LogoChoice summary={summary} />}
      <PluginSettingsSection plugin={plugin} />
      <PluginQualities plugin={plugin} />
      <PluginJobs summary={summary} jobs={plugin.jobs} />
      <PluginLog id={summary.id} />
    </>
  );
}
