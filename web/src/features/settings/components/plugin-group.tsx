import { PluginItem } from "@/features/settings/components/plugin-item";
import { SettingGroup } from "@/features/settings/components/setting-group";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/** The Plugins in one state, under that state as their `title`. */
export function PluginGroup({
  title,
  plugins,
  onFlip,
}: {
  title: string;
  plugins: PluginSummary[];
  onFlip: (plugin: PluginSummary) => void;
}) {
  return (
    <SettingGroup title={title}>
      <ul aria-label={title} className="-mb-2.5 flex flex-col">
        {plugins.map((plugin) => (
          <PluginItem key={plugin.id} plugin={plugin} onFlip={onFlip} />
        ))}
      </ul>
    </SettingGroup>
  );
}
