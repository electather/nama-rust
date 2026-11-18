import { ChildPermissions } from "@/shared/components/permissions-form/child-permissions";
import {
  type CheckboxProps,
  PermissionCheckbox,
} from "@/shared/components/permissions-form/permission-checkbox";
import type { PermissionChoice } from "@/shared/lib/permissions";

/** The top-level Permissions of `offer`, each with its children under it. */
export function PermissionChoices({
  offer,
  ...props
}: CheckboxProps & { offer: PermissionChoice[] }) {
  return (
    <ul className="mt-3 flex flex-col gap-3">
      {offer
        .filter((choice) => choice.parent === undefined)
        .map((choice) => (
          <li key={choice.permission}>
            <PermissionCheckbox choice={choice} {...props} />
            <ChildPermissions parent={choice.permission} {...props} />
          </li>
        ))}
    </ul>
  );
}
