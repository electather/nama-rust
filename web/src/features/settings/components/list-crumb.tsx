import { Link } from "@tanstack/react-router";

import { type Section, sectionSearch, sectionTitles } from "@/features/settings/lib/sections";

/** The crumb back to `section`'s list from what's open in place of it. */
export function ListCrumb({ section }: { section: Section }) {
  return (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      // The list's search is a subset of what's open's, so inclusive matching marks this current.
      activeOptions={{ exact: true }}
      className="rounded-sm text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {sectionTitles[section]}
    </Link>
  );
}
