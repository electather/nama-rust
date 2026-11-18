import { pluginPermissions } from "@/features/admin-plugins/lib/plugin-permissions";
import type { PluginPermission } from "@/gen/nama/v1/plugin_pb";

/**
 * The Plugin Permissions an Admin granted the Plugin, if any, as a line of a wrapping flex row.
 * One from a newer Server than this app knows is left out.
 */
export function GrantedPermissions({ granted }: { granted: PluginPermission[] }) {
  const labels = granted.flatMap((permission) => pluginPermissions[permission]?.label ?? []);
  if (labels.length === 0) {
    return null;
  }
  return (
    <p className="w-full text-sm text-pretty text-muted-foreground">Granted: {labels.join(", ")}</p>
  );
}
