import { Puzzle } from "lucide-react";

import { pluginName } from "@/shared/lib/plugins";

/**
 * A Plugin's logo on a neutral tile, or a generic puzzle piece when it has none. Only ever an image
 * from its URL (`src/features/admin-plugins/AGENTS.md`): as an image, SVG runs no script and loads nothing.
 */
export function PluginLogo({ plugin }: { plugin: { id: string; name: string; logoUrl: string } }) {
  const label = `${pluginName(plugin)} logo`;
  return (
    <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted">
      {plugin.logoUrl === "" ? (
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- our own icon, inline so it follows the colour scheme, which an <img> can't
        <Puzzle role="img" aria-label={label} className="size-5 text-muted-foreground" />
      ) : (
        <img src={plugin.logoUrl} alt={label} className="size-full object-contain" />
      )}
    </div>
  );
}
