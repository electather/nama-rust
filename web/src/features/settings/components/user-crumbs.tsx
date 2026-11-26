import { useSearch } from "@tanstack/react-router";

import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { sectionTitles } from "@/features/settings/lib/sections";
import { useUsers } from "@/shared/hooks/use-users";

/** Users, then the User `?user=` opens. An id no User has shows the list, as the pane does. */
export function UserCrumbs() {
  const asked = useSearch({ strict: false, select: ({ user }) => user });
  const open = useUsers().find((user) => Number(user.id) === asked);
  if (!open) {
    return <Crumbs current={sectionTitles.users} />;
  }
  return <Crumbs parent={<ListCrumb section="users" />} current={open.name} />;
}
