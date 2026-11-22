import { CreateUser } from "@/features/admin-users/components/create-user";
import { Invites } from "@/features/invites/components/invites";
import { DefaultPermissions } from "@/features/settings/components/default-permissions";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserGroup } from "@/features/settings/components/user-group";
import { userLabels } from "@/features/settings/lib/pane-labels";
import type { Permission, User } from "@/gen/nama/v1/sign_in_pb";

/** The groups of the list of Users: the Users, Create a User, the default Permissions and the Invites. */
export function UserListGroups({
  users,
  you,
  defaults,
  onChange,
}: {
  users: User[];
  you: User | undefined;
  defaults: Permission[];
  onChange: (changes: { permissions: Permission[] }) => void;
}) {
  return (
    <>
      <UserGroup users={users} you={you} />
      <SettingGroup setting="create" title={userLabels.create.label}>
        <CreateUser />
      </SettingGroup>
      <SettingGroup setting="defaults" title={userLabels.defaults.label}>
        <DefaultPermissions you={you} held={defaults} onChange={onChange} />
      </SettingGroup>
      <SettingGroup setting="invites" title={userLabels.invites.label}>
        <Invites you={you} />
      </SettingGroup>
    </>
  );
}
