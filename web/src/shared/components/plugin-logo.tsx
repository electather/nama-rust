import { Puzzle } from "lucide-react";

import { useServerUrl } from "@/shared/hooks/use-server-url";
import { pluginName } from "@/shared/lib/plugins";

const SIZES = {
  row: { tile: "size-6 rounded-sm", icon: "size-3.5" },
  // At the top of the Plugin's own view.
  large: { tile: "size-12 rounded-lg", icon: "size-6" },
};

/**
 * A Plugin's logo on a neutral tile, or a generic puzzle piece when it has none. Only ever an image
 * from its URL (`src/features/admin-plugins/AGENTS.md`): as an image, SVG runs no script and loads nothing.
 */
export function PluginLogo({
  plugin,
  large = false,
}: {
  plugin: { id: string; name: string; logoUrl: string };
  large?: boolean;
}) {
  const src = useServerUrl(plugin.logoUrl);
  const label = `${pluginName(plugin)} logo`;
  const size = SIZES[large ? "large" : "row"];
  return (
    <div
      className={`flex shrink-0 items-center justify-center overflow-hidden bg-muted ${size.tile}`}
    >
      {plugin.logoUrl === "" ? (
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- our own icon, inline so it follows the colour scheme, which an <img> can't
        <Puzzle role="img" aria-label={label} className={`text-muted-foreground ${size.icon}`} />
      ) : (
        <img src={src} alt={label} className="size-full object-contain" />
      )}
    </div>
  );
}
