import { PermissionCheckbox } from "@/features/admin-plugins/components/permission-checkbox";
import type { PluginPermission } from "@/gen/nama/v1/plugin_pb";

/** A checkbox for each Plugin Permission the Plugin requests. */
export function RequestedPermissions({ requested }: { requested: PluginPermission[] }) {
  return (
    <ul className="flex flex-col gap-3">
      {requested.map((permission) => (
        <li key={permission}>
          <PermissionCheckbox permission={permission} />
        </li>
      ))}
    </ul>
  );
}
