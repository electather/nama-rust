import { Link } from "@tanstack/react-router";
import { ChevronRight } from "lucide-react";

import { SectionIcon } from "@/features/settings/components/section-icon";
import { usePhoneWidth } from "@/features/settings/hooks/use-phone-width";
import { openingSection } from "@/features/settings/lib/history-state";
import { definitions } from "@/features/settings/lib/sections";
import { useSettingsAddress } from "@/shared/hooks/use-settings-address";
import { type Section, sectionSearch } from "@/shared/lib/settings-address";

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
  const { page } = useSettingsAddress();
  const marked = current && !phone;
  const { title, icon } = definitions[section];
  return (
    <Link
      to="."
      search={(rest) => ({ ...rest, ...sectionSearch(section) })}
      {...openingSection(page, phone)}
      aria-current={marked ? "page" : undefined}
      className={marked ? CURRENT : OTHER}
    >
      <SectionIcon icon={icon} marked={marked} />
      {title}
      <ChevronRight
        aria-hidden
        strokeWidth={1.5}
        className="ms-auto size-4 text-muted-foreground md:hidden rtl:rotate-180"
      />
    </Link>
  );
}
