import { useMutation } from "@connectrpc/connect-query";
import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import { DeviceList } from "@/components/device-list";
import { DeleteUser, ResetPassword } from "@/components/user-dialogs";
import { Button } from "@/components/ui/button";
import { Role, type User } from "@/gen/nama/v1/sign_in_pb";
import { UserService } from "@/gen/nama/v1/user_pb";
import { onlyAdminRefusal, roleName, usersKey } from "@/lib/users";

/** A User with their Role, opening to manage them and their Devices. */
export function UserRow({ user, isYou }: { user: User; isYou: boolean }) {
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
        <span className="text-sm text-muted-foreground">
          {roleName(user.role)}
          {isYou && " · You"}
        </span>
        <span className="ms-auto text-sm text-muted-foreground">{open ? "Close" : "Manage"}</span>
      </button>
      {open && (
        <div id={panel} className="flex flex-col gap-6 px-3 pt-2 pb-6">
          <UserActions user={user} isYou={isYou} />
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

function UserActions({ user, isYou }: { user: User; isYou: boolean }) {
  const queryClient = useQueryClient();
  // Demoting yourself makes the Server refuse the refetch, and the router then sees the new Role.
  const setRole = useMutation(UserService.method.setRole, {
    // Refused or not, the list may be out of date.
    onSettled: () => queryClient.invalidateQueries({ queryKey: usersKey }),
  });
  const otherRole = user.role === Role.ADMIN ? Role.MEMBER : Role.ADMIN;
  return (
    <div>
      {/* Pulled back by the buttons' own padding, so the first one's words line up with the name. */}
      <div className="-ms-4 flex flex-wrap gap-1">
        <Button
          variant="ghost"
          disabled={setRole.isPending}
          onClick={() => {
            setRole.mutate({ userId: user.id, role: otherRole });
          }}
        >
          Make {roleName(otherRole)}
        </Button>
        {/* The Server refuses to reset your own: changing it in Settings keeps this Device signed in. */}
        {!isYou && <ResetPassword user={user} />}
        <DeleteUser user={user} />
      </div>
      <p role="alert" className="mt-2 text-sm text-destructive empty:hidden">
        {setRole.error && onlyAdminRefusal(setRole.error, user.name, "change the Role")}
      </p>
    </div>
  );
}
