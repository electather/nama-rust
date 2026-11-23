import { useSetDefaultPermissions } from "@/features/admin-users/data/use-set-default-permissions";
import { PaneFrame } from "@/features/settings/components/pane-frame";
import { SaveToast } from "@/features/settings/components/save-toast";
import { UserListGroups } from "@/features/settings/components/user-list-groups";
import { useAutoSave } from "@/features/settings/hooks/use-auto-save";
import { refusedWords } from "@/features/settings/lib/refused-words";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { useDefaultPermissions } from "@/shared/hooks/use-default-permissions";

/**
 * Every User on the Server, each opening in place of the list, then a form to create one, the
 * Permissions and cap for remote playback new Users get, saved as they're changed, and the Invites.
 */
export function UserList({ users, you }: { users: User[]; you: User | undefined }) {
  const setDefaults = useSetDefaultPermissions();
  const defaults = useDefaultPermissions();
  const { values, refused, edit, dismiss } = useAutoSave(
    { permissions: defaults.permissions, cap: defaults.remotePlaybackLimit },
    async ({ permissions, cap }) => {
      await setDefaults.mutateAsync({ permissions, remotePlaybackLimit: cap });
    },
  );
  return (
    <>
      <PaneFrame section="users">
        <UserListGroups users={users} you={you} defaults={values} onChange={edit} />
      </PaneFrame>
      <SaveToast
        says={refusedWords(refused, { permissions: { label: "the default Permissions" } }).form}
        onDismiss={dismiss}
      />
    </>
  );
}
