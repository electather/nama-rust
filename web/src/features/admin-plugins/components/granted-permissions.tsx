import { pluginPermissions } from "@/features/admin-plugins/lib/plugin-permissions";
import type { PluginPermission } from "@/gen/nama/v1/plugin_pb";

/**
 * The Plugin Permissions the Plugin holds, if any, as a line of a wrapping flex row: those an Admin
 * granted, or for a `builtIn` one all it requests, as part of the Server. One from a newer Server
 * than this app knows is left out.
 */
export function GrantedPermissions({
  granted,
  builtIn,
}: {
  granted: PluginPermission[];
  builtIn: boolean;
}) {
  const labels = granted.flatMap((permission) => pluginPermissions[permission]?.label ?? []);
  if (labels.length === 0) {
    return null;
  }
  return (
    <p className="w-full text-sm text-pretty text-muted-foreground">
      {builtIn ? "Always holds, as part of the Server" : "Granted"}: {labels.join(", ")}
    </p>
  );
}
