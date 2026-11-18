import type { PersonalSettings as Personal } from "@/gen/nama/v1/user_pb";
import { PluginLogo } from "@/shared/components/plugin-logo";

/** A card's title: the Plugin's logo and name. */
export function CardHeading({ plugin, id }: { plugin: Personal; id: string }) {
  return (
    <div className="flex items-center gap-3">
      <PluginLogo plugin={{ id: plugin.pluginId, name: plugin.name, logoUrl: plugin.logoUrl }} />
      <h3 id={id} className="font-medium">
        {plugin.name}
      </h3>
    </div>
  );
}
