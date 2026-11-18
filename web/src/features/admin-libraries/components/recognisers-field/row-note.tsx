import { NotEnabled } from "@/features/admin-libraries/components/recognisers-field/not-enabled";
import type { PluginSummary } from "@/gen/nama/v1/plugin_pb";

/**
 * The note on a row for `plugin`: that it's off, which the Library skips until it's enabled, or,
 * once the Plugins are `listed`, that there's no such Plugin. Until the list arrives, no stored
 * Recogniser is known to be missing.
 */
export function RowNote({
  plugin,
  listed,
}: {
  plugin: PluginSummary | undefined;
  listed: boolean;
}) {
  if (plugin === undefined) {
    return listed ? <span className="text-muted-foreground"> · not available</span> : null;
  }
  return plugin.enabled ? null : <NotEnabled />;
}
