import { DeleteUser } from "@/features/admin-users/components/delete-user";
import { ResetPassword } from "@/features/admin-users/components/reset-password";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** Resetting `user`'s password, unless they're you, and deleting them. */
export function UserActions({ user, isYou }: { user: User; isYou: boolean }) {
  return (
    <div className="ms-auto flex flex-wrap gap-1">
      {/* The Server refuses to reset your own: changing it in Settings keeps this Device signed in. */}
      {!isYou && <ResetPassword user={user} />}
      <DeleteUser user={user} />
    </div>
  );
}
