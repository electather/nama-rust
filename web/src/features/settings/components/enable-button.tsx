import { GrantAndEnable } from "@/features/admin-plugins/components/grant-and-enable";
import { asksWhichToGrant } from "@/features/admin-plugins/lib/plugin-permissions";
import type { PluginSummary, Setting } from "@/gen/nama/v1/plugin_pb";
import { pluginName } from "@/shared/lib/plugins";
import { Button } from "@/shared/ui/button";

/**
 * The button that enables a disabled Plugin, the one thing its view is waiting on. Where it
 * requests Plugin Permissions it first asks which to grant. An enabled Plugin has none.
 */
export function EnableButton({
  summary,
  settings,
  pending,
  onEnable,
}: {
  summary: PluginSummary;
  settings: Setting[];
  pending: boolean;
  onEnable: () => void;
}) {
  if (summary.enabled) {
    return null;
  }
  if (asksWhichToGrant(summary)) {
    return <GrantAndEnable primary summary={summary} settings={settings} />;
  }
  return (
    <Button aria-label={`Enable ${pluginName(summary)}`} disabled={pending} onClick={onEnable}>
      Enable
    </Button>
  );
}
