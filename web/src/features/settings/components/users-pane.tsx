import { UserDetail } from "@/features/settings/components/user-detail";
import { UserList } from "@/features/settings/components/user-list";
import { useOpenUser } from "@/features/settings/hooks/use-open-user";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import { useUsers } from "@/shared/hooks/use-users";

/**
 * The Server's Users, for a User who manages them: the list of them, or in its place the one
 * `?user=<id>` names, to manage.
 */
export function UsersPane({ user: you }: { user: User | undefined }) {
  const users = useUsers();
  const open = useOpenUser();
  return open ? <UserDetail user={open} you={you} /> : <UserList users={users} you={you} />;
}
