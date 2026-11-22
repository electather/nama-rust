import { PluginSettings } from "@/features/admin-plugins/components/plugin-settings";
import type { PluginDetails } from "@/shared/lib/plugins";

/** The Settings section of a Plugin's view: its form. A Plugin with no settings has no section. */
export function PluginSettingsSection({ plugin }: { plugin: PluginDetails }) {
  if (plugin.settings.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="settings" className="mt-10">
      <PluginSettings plugin={plugin} />
    </section>
  );
}
