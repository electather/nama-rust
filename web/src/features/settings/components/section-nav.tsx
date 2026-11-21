import type { Ref } from "react";

import { SectionLink } from "@/features/settings/components/section-link";
import type { Section, SectionGroup } from "@/features/settings/lib/sections";

/**
 * The sections of Settings as links, the one open marked: under their groups' headings down the
 * side, or on a phone in one row that scrolls sideways.
 */
export function SectionNav({
  ref,
  groups,
  section,
}: {
  ref: Ref<HTMLElement>;
  groups: SectionGroup[];
  section: Section;
}) {
  return (
    <nav
      ref={ref}
      aria-label="Settings"
      className="flex min-w-0 flex-1 scrollbar-none gap-0.5 overflow-x-auto scroll-smooth motion-reduce:scroll-auto md:flex-none md:flex-col md:overflow-visible"
    >
      {groups.map((group) => (
        <div key={group.label} className="contents md:flex md:flex-col md:gap-0.5">
          <h3 className="px-2.5 pt-2.5 pb-1 text-xs text-muted-foreground max-md:hidden">
            {group.label}
          </h3>
          {group.items.map((item) => (
            <SectionLink key={item} section={item} current={item === section} />
          ))}
        </div>
      ))}
    </nav>
  );
}
