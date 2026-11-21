import { SetupSteps } from "@/features/admin-plugins/components/setup-steps";
import { pluginLinks } from "@/features/admin-plugins/lib/plugin-links";
import type { Plugin } from "@/gen/nama/v1/plugin_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/**
 * The About section of a Plugin's page: how to set it up, then its links and licence. A Plugin that
 * gives none of them, or only links that aren't http or https, has none. A view that gives the
 * links elsewhere, as Settings does in the header's "…", leaves them out with `links` off.
 */
export function PluginAbout({
  plugin,
  links: linked = true,
}: {
  plugin: Pick<Plugin, "setupMarkdown" | "homepageUrl" | "sourceUrl" | "license">;
  links?: boolean;
}) {
  const links = linked ? pluginLinks(plugin) : [];
  const hasFooter = links.length > 0 || plugin.license !== "";
  if (plugin.setupMarkdown.trim() === "" && !hasFooter) {
    return null;
  }
  return (
    <section aria-labelledby="about" className="mt-10 max-w-prose text-pretty">
      <h2 id="about" className="font-medium">
        About
      </h2>
      <SetupSteps markdown={plugin.setupMarkdown} />
      {hasFooter && (
        <p className="mt-4 flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
          {links.map(({ name, href }) => (
            <ExternalTextLink key={name} href={href}>
              {name}
            </ExternalTextLink>
          ))}
          {plugin.license !== "" && <span>{plugin.license} licence</span>}
        </p>
      )}
    </section>
  );
}
