import { CreateUser } from "@/features/admin-users/components/create-user";
import { CreateUserFromAccount } from "@/features/admin-users/components/create-user-from-account";
import { Invites } from "@/features/invites/components/invites";
import { DefaultPermissions } from "@/features/settings/components/default-permissions";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserGroup } from "@/features/settings/components/user-group";
import { userLabels } from "@/features/settings/lib/pane-labels";
import type { PlaybackLimit } from "@/gen/nama/v1/playback_pb";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";
import { useAccountSignIn } from "@/shared/hooks/use-account-sign-in";

/**
 * The groups of the list of Users: the Users, Create a User, on a Server Linked to the Hub from an
 * Account too, the default Permissions and the Invites.
 */
export function UserListGroups({
  users,
  you,
  defaults,
  onChange,
}: {
  users: User[];
  you: User | undefined;
  defaults: { permissions: Permission[]; cap: PlaybackLimit };
  onChange: (changes: { permissions: Permission[] } | { cap: PlaybackLimit }) => void;
}) {
  const linked = useAccountSignIn() === true;
  return (
    <>
      <UserGroup users={users} you={you} />
      <SettingGroup setting="create" title={userLabels.create.label}>
        <CreateUser />
      </SettingGroup>
      {linked && (
        <SettingGroup title="Create a User from an Account">
          <CreateUserFromAccount />
        </SettingGroup>
      )}
      <SettingGroup setting="defaults" title={userLabels.defaults.label}>
        <DefaultPermissions
          you={you}
          held={defaults.permissions}
          cap={defaults.cap}
          onChange={onChange}
        />
      </SettingGroup>
      <SettingGroup setting="invites" title={userLabels.invites.label}>
        <Invites you={you} />
      </SettingGroup>
    </>
  );
}
