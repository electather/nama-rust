import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenUser } from "@/features/settings/hooks/use-open-user";
import { sectionTitles } from "@/features/settings/lib/sections";

/** Users, then the User `?user=` opens, whose Users crumb goes back to the list. */
export function UserCrumbs() {
  const open = useOpenUser();
  if (!open) {
    return <Crumbs current={sectionTitles.users} />;
  }
  return <Crumbs parent={<ListCrumb section="users" />} current={open.name} />;
}
