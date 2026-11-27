import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { useOpenUser } from "@/features/settings/hooks/use-open-user";

/** Users, then the User `?user=` opens, whose Users crumb goes back to the list. */
export function UserCrumbs() {
  const { title } = useOpenSection();
  const open = useOpenUser();
  if (!open) {
    return <Crumbs current={title} />;
  }
  return <Crumbs parent={<ListCrumb />} current={open.name} />;
}
