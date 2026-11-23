import { useSetPermissions } from "@/features/admin-users/data/use-set-permissions";
import { onlyAdminRefusal } from "@/features/admin-users/lib/user-refusals";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/**
 * `user`'s Permissions and cap for remote playback, each change `you` make saved as it's made, and what the toast says of one
 * the Server refused.
 */
export function useUserPermissions(user: User, you: User | undefined) {
  const setPermissions = useSetPermissions(user, you);
  const { values, refused, edit, dismiss } = useAutoSave(
    { permissions: user.permissions, cap: user.remotePlaybackLimit },
    async ({ permissions, cap }) => {
      await setPermissions.mutateAsync({ userId: user.id, permissions, remotePlaybackLimit: cap });
    },
  );
  return {
    held: values.permissions,
    cap: values.cap,
    edit,
    dismiss,
    says: refused && onlyAdminRefusal(refused.failure, user.name, "save the Permissions"),
  };
}
