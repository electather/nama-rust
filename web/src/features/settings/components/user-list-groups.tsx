import { CreateUser } from "@/features/admin-users/components/create-user";
import { Invites } from "@/features/invites/components/invites";
import { DefaultPermissions } from "@/features/settings/components/default-permissions";
import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserGroup } from "@/features/settings/components/user-group";
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
      <SettingGroup title="Create a User">
        <CreateUser />
      </SettingGroup>
      <SettingGroup title="Default Permissions">
        <DefaultPermissions you={you} held={defaults} onChange={onChange} />
      </SettingGroup>
      <SettingGroup title="Invites">
        <Invites you={you} />
      </SettingGroup>
    </>
  );
}
