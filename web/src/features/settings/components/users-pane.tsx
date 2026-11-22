import { useSearch } from "@tanstack/react-router";

import { UserDetail } from "@/features/settings/components/user-detail";
import { UserList } from "@/features/settings/components/user-list";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { useUsers } from "@/shared/hooks/use-users";

/**
 * The Server's Users, for a User who manages them: the list of them, or in its place the one
 * `?user=<id>` names, to manage.
 */
export function UsersPane({ user: you }: { user: User | undefined }) {
  const users = useUsers();
  const asked = useSearch({ strict: false, select: ({ user }) => user });
  // An id no User has shows the list, as an address from before one was deleted would.
  const open = users.find((user) => Number(user.id) === asked);
  return open ? <UserDetail user={open} you={you} /> : <UserList users={users} you={you} />;
}
