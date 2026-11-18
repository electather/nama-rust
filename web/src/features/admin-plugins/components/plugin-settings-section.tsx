import { PluginSettings } from "@/features/admin-plugins/components/plugin-settings";
import type { PluginDetails } from "@/shared/lib/plugins";

/** The Settings section of a Plugin's page: its form, or that it has none. */
export function PluginSettingsSection({ plugin }: { plugin: PluginDetails }) {
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
