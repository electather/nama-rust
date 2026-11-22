import { Crumbs } from "@/features/settings/components/crumbs";
import { ListCrumb } from "@/features/settings/components/list-crumb";
import { useOpenJob } from "@/features/settings/hooks/use-open-job";
import { sectionTitles } from "@/features/settings/lib/sections";

/** Jobs, then the Job `?job=` opens, whose Jobs crumb goes back to the list. */
export function JobCrumbs() {
  const open = useOpenJob();
  if (!open) {
    return <Crumbs current={sectionTitles.jobs} />;
  }
  return <Crumbs parent={<ListCrumb section="jobs" />} current={open.name} />;
}
