import { SetupSteps } from "@/features/admin-plugins/components/setup-steps";
import { webUrl } from "@/features/admin-plugins/lib/setup-markdown";
import type { Plugin } from "@/gen/nama/v1/plugin_pb";
import { ExternalTextLink } from "@/shared/components/external-text-link";

/**
 * The About section of a Plugin's page: how to set it up, then its links and licence. A Plugin that
 * gives none of them, or only links that aren't http or https, has none.
 */
export function PluginAbout({
  plugin,
}: {
  plugin: Pick<Plugin, "setupMarkdown" | "homepageUrl" | "sourceUrl" | "license">;
}) {
  const links = [
    { name: "Homepage", href: webUrl(plugin.homepageUrl) },
    { name: "Source", href: webUrl(plugin.sourceUrl) },
  ].flatMap(({ name, href }) => (href === undefined ? [] : [{ name, href }]));
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
