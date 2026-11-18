import { PermissionsEditor } from "@/features/admin-users/components/permissions-editor";
import { UserActions } from "@/features/admin-users/components/user-actions";
import { UserDevices } from "@/features/admin-users/components/user-devices";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** What an open User row holds: its actions, the User's Permissions and their Devices. */
export function UserPanel({ id, user, you }: { id: string; user: User; you: User | undefined }) {
  return (
    <div id={id} className="flex flex-col gap-6 px-3 pt-2 pb-6">
      <UserActions user={user} isYou={user.id === you?.id} />
      <PermissionsEditor user={user} you={you} />
      <UserDevices user={user} />
    </div>
  );
}
