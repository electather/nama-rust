import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { useSensitiveMutation } from "@/components/confirm-its-you";
import { FormEnd } from "@/components/form-field";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { onSubmitted } from "@/lib/forms";
import { type PermissionChoice, isCovered, permissionChoices } from "@/lib/permissions";
import { onlyAdminRefusal, usersKey } from "@/lib/users";

/** `user`'s Permissions as checkboxes, with each one's children under it, saved together. */
export function PermissionsEditor({ user }: { user: User }) {
  const queryClient = useQueryClient();
  const [held, setHeld] = useState(user.permissions);
  // Removing your own Admin makes the Server refuse the refetch, and the router then sees your new
  // Permissions.
  const save = useSensitiveMutation(UserService.method.setPermissions, {
    // Refused or not, the list may be out of date.
    onSettled: () => queryClient.invalidateQueries({ queryKey: usersKey }),
  });
  const toggle = (permission: Permission, checked: boolean) => {
    setHeld(checked ? [...held, permission] : held.filter((each) => each !== permission));
  };
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={onSubmitted(() => {
        // A covered Permission is sent as it's held, so unchecking its parent later brings it back.
        save.mutate({ userId: user.id, permissions: held });
      })}
    >
      <fieldset>
        <legend className="text-sm font-medium">Permissions</legend>
        <ul className="mt-3 flex flex-col gap-3">
          {permissionChoices
            .filter((choice) => choice.parent === undefined)
            .map((choice) => (
              <li key={choice.permission}>
                <PermissionCheckbox choice={choice} held={held} onToggle={toggle} />
                <ChildPermissions parent={choice.permission} held={held} onToggle={toggle} />
              </li>
            ))}
        </ul>
      </fieldset>
      <FormEnd
        action={save.isPending ? "Saving…" : "Save Permissions"}
        variant="ghost"
        pending={save.isPending}
        done={save.isSuccess ? "Permissions saved." : undefined}
        refusal={
          save.error ? onlyAdminRefusal(save.error, user.name, "save the Permissions") : undefined
        }
      />
    </form>
  );
}

interface CheckboxProps {
  held: Permission[];
  onToggle: (permission: Permission, checked: boolean) => void;
}

function ChildPermissions({ parent, ...props }: CheckboxProps & { parent: Permission }) {
  const children = permissionChoices.filter((choice) => choice.parent === parent);
  if (children.length === 0) return null;
  return (
    <ul className="mt-3 ms-7 flex flex-col gap-3">
      {children.map((choice) => (
        <li key={choice.permission}>
          <PermissionCheckbox choice={choice} {...props} />
        </li>
      ))}
    </ul>
  );
}

/** One Permission, shown checked and fixed while another the User holds covers it. */
function PermissionCheckbox({
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
