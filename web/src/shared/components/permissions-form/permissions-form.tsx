import { type ComponentProps, type ReactNode, useState } from "react";

import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { FormEnd } from "@/shared/components/form-field/form-end";
import { PermissionChoices } from "@/shared/components/permissions-form/permission-choices";
import { onSubmitted } from "@/shared/lib/forms";
import { type PermissionChoice, toggledPermissions } from "@/shared/lib/permissions";

/**
 * The Permissions of `offer` as checkboxes, with each one's children under it, after any other
 * `fields`. `onSave` gets those checked together, and what was typed.
 */
export function PermissionsForm({
  initial,
  offer,
  fields,
  onSave,
  end,
}: {
  initial: Permission[];
  offer: PermissionChoice[];
  fields?: ReactNode;
  onSave: (permissions: Permission[], typed: FormData) => void;
  end: ComponentProps<typeof FormEnd>;
}) {
  const [held, setHeld] = useState(initial);
  const toggle = (permission: Permission, checked: boolean) => {
    setHeld(toggledPermissions(held, permission, checked));
  };
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={onSubmitted((typed) => {
        // A covered Permission is sent as it's held, so unchecking its parent later brings it back.
        onSave(held, typed);
      })}
    >
      {fields}
      <fieldset>
        <legend className="text-sm font-medium">Permissions</legend>
        <PermissionChoices offer={offer} held={held} onToggle={toggle} />
      </fieldset>
      <FormEnd variant="ghost" {...end} />
    </form>
  );
}
