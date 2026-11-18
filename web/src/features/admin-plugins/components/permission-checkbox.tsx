import { useId } from "react";

import { pluginPermissions } from "@/features/admin-plugins/lib/plugin-permissions";
import type { PluginPermission } from "@/gen/nama/v1/plugin_pb";

/** One requested Plugin Permission, unchecked until the Admin grants it. */
export function PermissionCheckbox({ permission }: { permission: PluginPermission }) {
  const id = useId();
  const { label, about } = pluginPermissions[permission] ?? {
    label: `Permission ${String(permission)}`,
    about: "This app doesn’t know it yet.",
  };
  return (
    <div className="flex gap-3">
      <input
        id={id}
        type="checkbox"
        name="permission"
        value={permission}
        aria-describedby={`${id}-about`}
        className="mt-0.5 size-4 shrink-0 accent-foreground"
      />
      <div className="text-sm">
        <label htmlFor={id} className="font-medium">
          {label}
        </label>
        <p id={`${id}-about`} className="text-muted-foreground">
          {about}
        </p>
      </div>
    </div>
  );
}
