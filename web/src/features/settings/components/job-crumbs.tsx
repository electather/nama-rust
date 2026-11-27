import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenSection } from "@/features/settings/hooks/open-section";
import { useOpenJob } from "@/features/settings/hooks/use-open-job";

/** Jobs, then the Job `?job=` opens, whose Jobs crumb goes back to the list. */
export function JobCrumbs() {
  const { title } = useOpenSection();
  const open = useOpenJob();
  if (!open) {
    return <Crumbs current={title} />;
  }
  return <Crumbs parent={<ListCrumb />} current={open.name} />;
}
