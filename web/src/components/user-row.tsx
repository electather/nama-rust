import { useId, useState } from "react";

import { DeviceList } from "@/components/device-list";
import { PermissionsEditor } from "@/components/permissions-editor";
import { DeleteUser, ResetPassword } from "@/components/user-dialogs";
import { Permission, type User } from "@/gen/nama/v1/sign_in_pb";
import { may } from "@/lib/permissions";

/**
 * A User, marked if an Admin or `you`, opening to manage them, their Permissions and their Devices.
 */
export function UserRow({ user, you }: { user: User; you: User | undefined }) {
  const isYou = user.id === you?.id;
  const [open, setOpen] = useState(false);
  const panel = useId();
  return (
    <li className="border-b last:border-b-0">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panel}
        onClick={() => {
          setOpen(!open);
        }}
        className="flex min-h-14 w-full items-center gap-3 rounded-lg px-3 text-start outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
      >
        <span className="truncate font-medium">{user.name}</span>
        <span className="text-sm text-muted-foreground">{aboutUser(user, isYou)}</span>
        <span className="ms-auto text-sm text-muted-foreground">{open ? "Close" : "Manage"}</span>
      </button>
      {open && (
        <div id={panel} className="flex flex-col gap-6 px-3 pt-2 pb-6">
          <UserActions user={user} isYou={isYou} />
          <PermissionsEditor user={user} you={you} />
          <div>
            <h3 className="text-sm font-medium">Devices</h3>
            <div className="mt-2">
              <DeviceList user={user} />
            </div>
          </div>
        </div>
      )}
    </li>
  );
}

/** What the row says beside a User's name: whether they're an Admin, and whether they're you. */
function aboutUser(user: User, isYou: boolean) {
  return [may(user, Permission.ADMIN) && "Admin", isYou && "You"].filter(Boolean).join(" · ");
}

function UserActions({ user, isYou }: { user: User; isYou: boolean }) {
  return (
    // Pulled back by the buttons' own padding, so the first one's words line up with the name.
    <div className="-ms-4 flex flex-wrap gap-1">
      {/* The Server refuses to reset your own: changing it in Settings keeps this Device signed in. */}
      {!isYou && <ResetPassword user={user} />}
      <DeleteUser user={user} />
    </div>
  );
}
