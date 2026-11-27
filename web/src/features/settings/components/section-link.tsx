import { Link, useSearch } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import { SectionIcon } from "@/features/settings/components/section-icon";
import { usePhoneWidth } from "@/features/settings/hooks/use-phone-width";
import { openingSection } from "@/features/settings/lib/history-state";
import { type Section, sectionSearch, sectionTitles } from "@/features/settings/lib/sections";

const LINK =
  "flex h-11 flex-none items-center gap-2.5 rounded-sm px-2 text-sm whitespace-nowrap transition-colors outline-none focus-visible:ring-3 focus-visible:ring-ring/50 motion-reduce:transition-none md:h-7";
// The open section is filled and bold, so it isn't marked by colour alone.
const CURRENT = `${LINK} bg-popover font-medium text-foreground`;
// On a phone every row reads as a choice to open, in full ink.
const OTHER = `${LINK} text-muted-foreground hover:bg-muted hover:text-foreground max-md:text-foreground`;

/**
 * A section's link: in the sidebar, marked while it's the one open; on a phone, a row of the list
 * of sections that opens the section's page (`openingSection`).
 */
export function SectionLink({ section, current }: { section: Section; current: boolean }) {
  const phone = usePhoneWidth();
  const settings = useSearch({ strict: false, select: (search) => search.settings });
  const marked = current && !phone;
  return (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      {...openingSection(settings, phone)}
      aria-current={marked ? "page" : undefined}
      className={marked ? CURRENT : OTHER}
    >
      <SectionIcon section={section} marked={marked} />
      {sectionTitles[section]}
      <ChevronRight
        aria-hidden
        strokeWidth={1.5}
        className="ms-auto size-4 text-muted-foreground md:hidden rtl:rotate-180"
      />
    </Link>
  );
}
