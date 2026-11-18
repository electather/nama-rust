import { UserPermissionsForm } from "@/features/admin-users/components/user-permissions-form";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { mayChangePermissions } from "@/shared/lib/permissions";

/** `user`'s Permissions as checkboxes, or nothing when `you` may not change them. */
export function PermissionsEditor({ user, you }: { user: User; you: User | undefined }) {
  return mayChangePermissions(you, user) ? <UserPermissionsForm user={user} you={you} /> : null;
}
