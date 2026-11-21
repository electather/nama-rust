import { webUrl } from "@/features/admin-plugins/lib/setup-markdown";
import type { Plugin } from "@/gen/nama/v1/plugin_pb";

/** A Plugin's links out, to its homepage and its source: only the ones that are http or https. */
export function pluginLinks(plugin: Pick<Plugin, "homepageUrl" | "sourceUrl">) {
  return [
    { name: "Homepage", href: webUrl(plugin.homepageUrl) },
    { name: "Source", href: webUrl(plugin.sourceUrl) },
  ].flatMap(({ name, href }) => (href === undefined ? [] : [{ name, href }]));
}
