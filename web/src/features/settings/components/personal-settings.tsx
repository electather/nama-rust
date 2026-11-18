import { PluginCard } from "@/features/settings/components/plugin-card";
import { usePersonalSettings } from "@/features/settings/data/use-personal-settings";

/**
 * The signed-in User's own Personal settings, a card per enabled Plugin that declares any, such as
 * their ntfy topic. Nothing while no Plugin does.
 */
export function PersonalSettings() {
  const { data, dataUpdatedAt } = usePersonalSettings();
  if (!data || data.plugins.length === 0) {
    return null;
  }
  return (
    <section aria-labelledby="personal" className="mt-12 max-w-md">
      <h2 id="personal" className="font-medium">
        Personal settings
      </h2>
      <p className="mt-1 text-sm text-muted-foreground">
        Only you see these, and a Plugin uses them only for you.
      </p>
      <ul className="mt-4 flex flex-col gap-4">
        {data.plugins.map((plugin) => (
          <PluginCard key={plugin.pluginId} plugin={plugin} listedAt={dataUpdatedAt} />
        ))}
      </ul>
    </section>
  );
}
