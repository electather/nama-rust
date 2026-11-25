import { useSetPermissions } from "@/features/admin-users/data/use-set-permissions";
import { useSetUserAccess } from "@/features/admin-users/data/use-set-user-access";
import { onlyAdminRefusal } from "@/features/admin-users/lib/user-refusals";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** What a refused save of each setting tried to do, as its toast says. */
const ATTEMPTS: Record<string, string> = {
  maximumAge: "save the Maximum Age",
  libraryAccess: "save the Libraries",
};

/**
 * `user`'s Permissions, cap for remote playback, Maximum Age and Libraries, each change `you` make
 * saved as it's made, and what the toast says of one the Server refused.
 */
export function useUserSettings(user: User, you: User | undefined) {
  const setPermissions = useSetPermissions(user, you);
  const { setMaximumAge, setLibraryAccess } = useSetUserAccess();
  const { values, refused, edit, dismiss } = useAutoSave(
    {
      permissions: user.permissions,
      cap: user.remotePlaybackLimit,
      maximumAge: user.maximumAge,
      libraryAccess: user.libraryAccess,
    },
    async ({ permissions, cap, maximumAge, libraryAccess }, changed) => {
      const userId = user.id;
      // Each control changes one setting. Maximum Age and Libraries each have an RPC of their own;
      // Permissions and the cap share `SetPermissions`.
      if (changed.includes("maximumAge")) {
        await setMaximumAge.mutateAsync({ userId, maximumAge });
      } else if (changed.includes("libraryAccess")) {
        await setLibraryAccess.mutateAsync({ userId, libraryAccess });
      } else {
        await setPermissions.mutateAsync({ userId, permissions, remotePlaybackLimit: cap });
      }
    },
  );
  const attempt = ATTEMPTS[refused?.keys[0] ?? ""] ?? "save the Permissions";
  return {
    values,
    edit,
    dismiss,
    says: refused && onlyAdminRefusal(refused.failure, user.name, attempt),
  };
}
