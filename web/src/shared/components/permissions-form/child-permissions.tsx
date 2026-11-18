import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import {
  type CheckboxProps,
  PermissionCheckbox,
} from "@/shared/components/permissions-form/permission-checkbox";
import { permissionChoices } from "@/shared/lib/permissions";

/** The Permissions under `parent`, indented, or nothing when it has none. */
export function ChildPermissions({ parent, ...props }: CheckboxProps & { parent: Permission }) {
  const children = permissionChoices.filter((choice) => choice.parent === parent);
  if (children.length === 0) {
    return null;
  }
  return (
    <ul className="ms-7 mt-3 flex flex-col gap-3">
      {children.map((choice) => (
        <li key={choice.permission}>
          <PermissionCheckbox choice={choice} {...props} />
        </li>
      ))}
    </ul>
  );
}
