import { Link } from "@tanstack/react-router";

import { useOpenSection } from "@/features/settings/hooks/open-section";
import { sectionSearch } from "@/shared/lib/settings-address";

/** The crumb back to the open Section's list from what's open in place of it. */
export function ListCrumb() {
  const { section, title } = useOpenSection();
  return (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      // The list's search is a subset of what's open's, so inclusive matching marks this current.
      activeOptions={{ exact: true }}
      className="rounded-sm text-muted-foreground outline-none hover:text-foreground hover:underline focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      {title}
    </Link>
  );
}
