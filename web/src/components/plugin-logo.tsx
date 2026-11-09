import { Puzzle } from "lucide-react";

/**
 * A Plugin's logo on a neutral tile, or a generic puzzle piece when it has none. The logo is the
 * Plugin's own untrusted SVG, so it is only ever an image from a data URL: SVG shown as an image
 * runs no script and loads nothing, which it would inline in the page.
 */
export function PluginLogo({ plugin }: { plugin: { id: string; logoSvg: string } }) {
  const label = `${plugin.id} logo`;
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
