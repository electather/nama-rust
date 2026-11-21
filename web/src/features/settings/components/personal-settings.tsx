import { PluginCard } from "@/features/settings/components/plugin-card";
import { usePersonalSettings } from "@/features/settings/data/use-personal-settings";

/**
 * The signed-in User's own Personal settings, a card per enabled Plugin that declares any, such as
 * their ntfy topic. Nothing while no Plugin does.
 */
export function PersonalSettings() {
  const { data } = usePersonalSettings();
  if (data.plugins.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="personal" className="flex flex-col gap-3.5">
      <div>
        <h4 id="personal" className="font-medium">
          Personal settings
        </h4>
        <p className="text-xs text-muted-foreground">
          Only you see these, and a Plugin uses them only for you.
        </p>
      </div>
      <ul className="flex flex-col gap-3.5">
        {data.plugins.map((plugin) => (
          <PluginCard key={plugin.pluginId} plugin={plugin} />
        ))}
      </ul>
    </section>
  );
}
