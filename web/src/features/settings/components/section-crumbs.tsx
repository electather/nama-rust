import { Link } from "@tanstack/react-router";

import { Crumbs } from "@/features/settings/components/crumbs";
import { type Section, sectionSearch, sectionTitles } from "@/features/settings/lib/sections";

/**
 * `section`, then the `open` thing in it, if any, whose crumb for the section goes back to its
 * list. Without one, the trail is the section alone.
 */
export function SectionCrumbs({ section, open }: { section: Section; open: string | undefined }) {
  if (open === undefined) {
    return <Crumbs current={sectionTitles[section]} />;
  }
  const back = (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      // The list's search is a subset of the open thing's, so inclusive matching marks this current.
      activeOptions={{ exact: true }}
      className="rounded-sm text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {sectionTitles[section]}
    </Link>
  );
  return <Crumbs parent={back} current={open} />;
}
