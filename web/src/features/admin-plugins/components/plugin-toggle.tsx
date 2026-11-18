import { Ban } from "lucide-react";

import { EnableOrDisable } from "@/features/admin-plugins/components/enable-or-disable";
import { GrantAndEnable } from "@/features/admin-plugins/components/grant-and-enable";
import type { PluginSummary, Setting } from "@/gen/nama/v1/plugin_pb";
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
  // Enabling a Plugin that requests Plugin Permissions first asks which to grant.
  if (!summary.enabled && summary.requestedPermissions.length > 0) {
    return <GrantAndEnable summary={summary} settings={settings} />;
  }
  return <EnableOrDisable summary={summary} settings={settings} />;
}
