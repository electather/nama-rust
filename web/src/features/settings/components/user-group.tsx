import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserItem } from "@/features/settings/components/user-item";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** The Users, under "Users", `you` among them marked. */
export function UserGroup({ users, you }: { users: User[]; you: User | undefined }) {
  return (
    <SettingGroup title="Users">
      <ul aria-label="Users" className="-mb-3.5 flex flex-col">
        {users.map((user) => (
          <UserItem key={user.id} user={user} isYou={user.id === you?.id} />
        ))}
      </ul>
    </SettingGroup>
  );
}
