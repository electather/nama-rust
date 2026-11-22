import type { ReactNode } from "react";

import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { PermissionChoices } from "@/shared/components/permissions-form/permission-choices";
import { permissionsToOffer, toggledPermissions } from "@/shared/lib/permissions";

/**
 * The Permissions `you` may give as checkboxes, `held` checked, under what `children` say of them.
 * Each one toggled goes to `onChange` with the rest, to save at once.
 */
export function PermissionsFieldset({
  legend,
  you,
  held,
  onChange,
  children,
}: {
  /** Said only to screen readers: the group it's in already shows its title. */
  legend: string;
  you: User | undefined;
  held: Permission[];
  onChange: (changes: { permissions: Permission[] }) => void;
  children?: ReactNode;
}) {
  return (
    <fieldset>
      <legend className="sr-only">{legend}</legend>
      {children}
      <PermissionChoices
        offer={permissionsToOffer(you)}
        held={held}
        onToggle={(permission, checked) => {
          onChange({ permissions: toggledPermissions(held, permission, checked) });
        }}
      />
    </fieldset>
  );
}
