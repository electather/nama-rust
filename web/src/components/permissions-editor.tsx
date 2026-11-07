import { useMutation, useSuspenseQuery } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { type ComponentProps, useId, useState } from "react";

import { FormEnd } from "@/components/form-field";
import { useSetPermissions } from "@/components/use-set-permissions";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { onSubmitted, refusal } from "@/lib/forms";
import {
  type PermissionChoice,
  isCovered,
  mayChangePermissions,
  permissionChoices,
  permissionsToOffer,
} from "@/lib/permissions";
import { defaultPermissionsKey, onlyAdminRefusal } from "@/lib/users";

/** `user`'s Permissions as checkboxes, or nothing when `you` may not change them. */
export function PermissionsEditor({ user, you }: { user: User; you: User | undefined }) {
  return mayChangePermissions(you, user) ? <UserPermissionsForm user={user} you={you} /> : null;
}

function UserPermissionsForm({ user, you }: { user: User; you: User | undefined }) {
  const save = useSetPermissions(user, you);
  return (
    <PermissionsForm
      initial={user.permissions}
      you={you}
      onSave={(permissions) => {
        save.mutate({ userId: user.id, permissions });
      }}
      end={{
        action: save.isPending ? "Saving…" : "Save Permissions",
        pending: save.isPending,
        done: save.isSuccess ? "Permissions saved." : undefined,
        refusal: save.error
          ? onlyAdminRefusal(save.error, user.name, "save the Permissions")
          : undefined,
      }}
    />
  );
}

/** The Permissions new Users get, named, opening to change them as `you` may. */
export function DefaultPermissionsEditor({ you }: { you: User | undefined }) {
  const { data } = useSuspenseQuery(UserService.method.getDefaultPermissions, {});
  const [open, setOpen] = useState(false);
  const panel = useId();
  return (
    <>
      <div className="mt-2 flex items-baseline gap-3">
        <p className="text-sm">{labelsOf(data.permissions)}</p>
        <button
          type="button"
          aria-expanded={open}
          aria-controls={panel}
          onClick={() => {
            setOpen(!open);
          }}
          className="ms-auto rounded-sm text-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        >
          {open ? "Close" : "Change"}
        </button>
      </div>
      {open && (
        <div id={panel} className="mt-4">
          <DefaultPermissionsForm initial={data.permissions} you={you} />
        </div>
      )}
    </>
  );
}

function DefaultPermissionsForm({
  initial,
  you,
}: {
  initial: Permission[];
  you: User | undefined;
}) {
  const queryClient = useQueryClient();
  const save = useMutation(UserService.method.setDefaultPermissions, {
    onSettled: () => queryClient.invalidateQueries({ queryKey: defaultPermissionsKey }),
  });
  return (
    <PermissionsForm
      initial={initial}
      you={you}
      onSave={(permissions) => {
        save.mutate({ permissions });
      }}
      end={{
        action: save.isPending ? "Saving…" : "Save default Permissions",
        pending: save.isPending,
        done: save.isSuccess ? "Default Permissions saved." : undefined,
        refusal: save.error
          ? refusal(save.error, "save the default Permissions", {}).form
          : undefined,
      }}
    />
  );
}

/** `permissions` by their labels, in the order the Users page lists them, or "None". */
function labelsOf(permissions: Permission[]) {
  const labels = permissionChoices
    .filter((choice) => permissions.includes(choice.permission))
    .map((choice) => choice.label);
  return labels.length === 0 ? "None" : labels.join(" · ");
}

/** Permissions as checkboxes, with each one's children under it, which `onSave` gets together. */
function PermissionsForm({
  initial,
  you,
  onSave,
  end,
}: {
  initial: Permission[];
  you: User | undefined;
  onSave: (permissions: Permission[]) => void;
  end: ComponentProps<typeof FormEnd>;
}) {
  const [held, setHeld] = useState(initial);
  const toggle = (permission: Permission, checked: boolean) => {
    setHeld(checked ? [...held, permission] : held.filter((each) => each !== permission));
  };
  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={onSubmitted(() => {
        // A covered Permission is sent as it's held, so unchecking its parent later brings it back.
        onSave(held);
      })}
    >
      <fieldset>
        <legend className="text-sm font-medium">Permissions</legend>
        <ul className="mt-3 flex flex-col gap-3">
          {permissionsToOffer(you)
            .filter((choice) => choice.parent === undefined)
            .map((choice) => (
              <li key={choice.permission}>
                <PermissionCheckbox choice={choice} held={held} onToggle={toggle} />
                <ChildPermissions parent={choice.permission} held={held} onToggle={toggle} />
              </li>
            ))}
        </ul>
      </fieldset>
      <FormEnd variant="ghost" {...end} />
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
