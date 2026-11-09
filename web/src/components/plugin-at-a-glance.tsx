// What both the Plugins list and a Plugin's page show of it, kept in one module so each page stays
// within oxlint's `import/max-dependencies`.
import { Activity, Bell, Film, KeyRound, Link, Puzzle, ScanSearch, Server, Tv } from "lucide-react";

import type { Shown } from "@/components/state-label";
import { ExtensionPoint } from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/lib/plugins";

/** Each Extension Point in plain words, with the icon the web picks for it. */
const points: Partial<Record<ExtensionPoint, Shown>> = {
  [ExtensionPoint.MOVIE_FULFILLER]: { label: "Fulfiller for Movies", Icon: Film },
  [ExtensionPoint.SHOW_FULFILLER]: { label: "Fulfiller for Shows", Icon: Tv },
  [ExtensionPoint.NOTIFIER]: { label: "Notifier", Icon: Bell },
  [ExtensionPoint.CONNECTOR]: { label: "Connector", Icon: Link },
  [ExtensionPoint.AUTHENTICATOR]: { label: "Authenticator", Icon: KeyRound },
  [ExtensionPoint.METADATA_PROVIDER]: { label: "Metadata Provider", Icon: ScanSearch },
  [ExtensionPoint.EVENTS_OBSERVER]: { label: "Observes Events", Icon: Activity },
  [ExtensionPoint.SERVER_STARTED_OBSERVER]: { label: "Observes Server start", Icon: Server },
};

/**
 * The Extension Points a Plugin takes part at, each an icon with its label below it. One from a
 * newer Server than this app knows is left out.
 */
export function ExtensionPoints({ extensionPoints }: { extensionPoints: ExtensionPoint[] }) {
  const shown = extensionPoints.flatMap((point) => points[point] ?? []);
  if (shown.length === 0) return null;
  return (
    <ul aria-label="Extension Points" className="mt-3 flex flex-wrap gap-x-5 gap-y-3">
      {shown.map(({ label, Icon }) => (
        <li key={label} className="flex flex-col items-start gap-1 text-sm text-muted-foreground">
          <Icon aria-hidden className="size-5" />
          {label}
        </li>
      ))}
    </ul>
  );
}

/**
 * A Plugin's logo on a neutral tile, or a generic puzzle piece when it has none. Only ever an image
 * from a data URL (`web/AGENTS.md`): as an image, SVG runs no script and loads nothing.
 */
export function PluginLogo({ plugin }: { plugin: { id: string; name: string; logoSvg: string } }) {
  const label = `${pluginName(plugin)} logo`;
  return (
    <div className="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted">
      {plugin.logoSvg === "" ? (
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role -- our own icon, inline so it follows the colour scheme, which an <img> can't
        <Puzzle role="img" aria-label={label} className="size-5 text-muted-foreground" />
      ) : (
        <img src={svgDataUrl(plugin.logoSvg)} alt={label} className="size-full" />
      )}
    </div>
  );
}

/** `svg` as a `data:image/svg+xml` URL, percent-encoded since SVG holds `#` and quotes. */
function svgDataUrl(svg: string) {
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
