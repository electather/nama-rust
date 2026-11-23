import { SettingGroup } from "@/features/settings/components/setting-group";
import { UserItem } from "@/features/settings/components/user-item";
import { useUsersWhoCantSignIn } from "@/features/settings/data/use-users-who-cant-sign-in";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/** The Users, under "Users", `you` among them marked, and those who can't sign in. */
export function UserGroup({ users, you }: { users: User[]; you: User | undefined }) {
  const cantSignIn = useUsersWhoCantSignIn();
  return (
    <SettingGroup title="Users">
      <ul aria-label="Users" className="-mb-3.5 flex flex-col">
        {users.map((user) => (
          <UserItem
            key={user.id}
            user={user}
            isYou={user.id === you?.id}
            cantSignIn={cantSignIn.includes(user.id)}
          />
        ))}
      </ul>
    </SettingGroup>
  );
}
