import { useId } from "react";

import type { Permission } from "@/gen/nama/v1/sign_in_pb";
import { type PermissionChoice, isCovered } from "@/shared/lib/permissions";

/** The Permissions checked so far, and what checking or unchecking one does. */
export interface CheckboxProps {
  held: Permission[];
  onToggle: (permission: Permission, checked: boolean) => void;
}

/** One Permission, shown checked and fixed while another the User holds covers it. */
export function PermissionCheckbox({
  choice: { permission, label, about },
  held,
  onToggle,
}: CheckboxProps & { choice: PermissionChoice }) {
  const id = useId();
  const covered = isCovered(held, permission);
  return (
    <div className="flex gap-3">
      <input
        id={id}
        type="checkbox"
        checked={covered || held.includes(permission)}
        disabled={covered}
        aria-describedby={`${id}-about`}
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
      </div>
    </div>
  );
}
