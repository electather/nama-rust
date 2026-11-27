import { useId } from "react";

import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type PermissionChoice, isCovered, toggledPermissions } from "@/shared/lib/permissions";

/** The Permissions checked so far, and what checking or unchecking one does. */
export interface CheckboxProps {
  held: Permission[];
  onToggle: (permission: Permission, checked: boolean) => void;
  /** Why the Permissions a toggle would leave can't be held, if they can't. */
  refuses?: ((permissions: Permission[]) => string | undefined) | undefined;
}

/**
 * One Permission, shown checked and fixed while another the User holds covers it, and fixed, saying
 * why, while `refuses` refuses what toggling it would leave.
 */
export function PermissionCheckbox({
  choice: { permission, label, about },
  held,
  onToggle,
  refuses,
}: CheckboxProps & { choice: PermissionChoice }) {
  const id = useId();
  const covered = isCovered(held, permission);
  const checked = covered || held.includes(permission);
  const why = refuses?.(toggledPermissions(held, permission, !checked));
  return (
    <div className="flex gap-3">
      <input
        id={id}
        type="checkbox"
        checked={checked}
        disabled={covered || why !== undefined}
        aria-describedby={`${id}-about ${id}-why`}
        onChange={(event) => {
          onToggle(permission, event.currentTarget.checked);
        }}
        className="mt-0.5 size-4 shrink-0 accent-foreground"
      />
      <div className="text-sm">
        <label htmlFor={id} className="font-medium">
          {label}
        </label>
        <p id={`${id}-about`} className="text-muted-foreground empty:hidden">
          {about}
        </p>
        <p id={`${id}-why`} className="text-muted-foreground empty:hidden">
          {why}
        </p>
      </div>
    </div>
  );
}
