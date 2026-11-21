import { Ban } from "lucide-react";

import { EnableOrDisable } from "@/features/admin-plugins/components/enable-or-disable";
import { GrantAndEnable } from "@/features/admin-plugins/components/grant-and-enable";
import { PluginKind, type PluginSummary, type Setting } from "@/gen/nama/v1/plugin_pb";
import { StateLabel } from "@/shared/components/state-label";

/**
 * Whether the Plugin `summary` is enabled, a button to change that, and why the Server refused to,
 * for a wrapping flex row. A Plugin the Server refused only says so. A refusal names a setting by
 * its label in `settings`, or by its key without them, as on the list.
 */
export function PluginToggle({
  summary,
  settings,
}: {
  summary: PluginSummary;
  settings?: Setting[];
}) {
  if (summary.refusedReason !== "") {
    return <StateLabel label="Refused" Icon={Ban} />;
  }
  if (asksWhichToGrant(summary)) {
    return <GrantAndEnable summary={summary} settings={settings} />;
  }
  return <EnableOrDisable summary={summary} settings={settings} />;
}

/**
 * Whether enabling `summary` first asks which Plugin Permissions to grant: a disabled Installed
 * Plugin's that requests some. A Built-in one always holds all it requests (ADR 0079).
 */
function asksWhichToGrant(summary: PluginSummary) {
  return (
    !summary.enabled &&
    summary.kind !== PluginKind.BUILT_IN &&
    summary.requestedPermissions.length > 0
  );
}
