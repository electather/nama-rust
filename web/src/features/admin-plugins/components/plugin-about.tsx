import { SetupSteps } from "@/features/admin-plugins/components/setup-steps";
import type { Plugin } from "@/gen/nama/v1/plugin_pb";

/**
 * The About section of a Plugin's view: how to set it up, then its licence. A Plugin that gives
 * neither has none. Its links are in the header's "…".
 */
export function PluginAbout({ plugin }: { plugin: Pick<Plugin, "setupMarkdown" | "license"> }) {
  if (plugin.setupMarkdown.trim() === "" && plugin.license === "") {
    return null;
  }
  return (
    <section aria-labelledby="about" className="mt-10 max-w-prose text-pretty">
      <h2 id="about" className="font-medium">
        About
      </h2>
      <SetupSteps markdown={plugin.setupMarkdown} />
      {plugin.license !== "" && (
        <p className="mt-4 text-sm text-muted-foreground">{plugin.license} licence</p>
      )}
    </section>
  );
}
