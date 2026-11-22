import { useRef, useState } from "react";

import { ScrollSections } from "@/features/settings/components/scroll-sections";
import { SectionNav } from "@/features/settings/components/section-nav";
import { SettingsSearch } from "@/features/settings/components/settings-search";
import { type Section, type SectionGroup, matching } from "@/features/settings/lib/sections";

/**
 * Where a section of Settings is picked: a searchable list down the side, or on a phone a strip of
 * them under the title, which its arrows scroll.
 */
export function SettingsSide({ groups, section }: { groups: SectionGroup[]; section: Section }) {
  const [typed, setTyped] = useState("");
  const strip = useRef<HTMLElement>(null);
  const found = matching(groups, typed);
  return (
    <div className="flex shrink-0 items-center gap-0.5 px-1 md:w-48 md:flex-col md:items-stretch md:gap-2.5 md:overflow-y-auto md:bg-sidebar md:px-2 md:py-3 lg:w-52">
      <SettingsSearch typed={typed} onType={setTyped} />
      <ScrollSections strip={strip} to="left" />
      <SectionNav ref={strip} groups={found} section={section} />
      <ScrollSections strip={strip} to="right" />
      {found.length === 0 && (
        <p className="px-2 text-sm text-muted-foreground max-md:hidden">
          No settings match “{typed.trim()}”.
        </p>
      )}
    </div>
  );
}
