import { type ReactNode, useId, useRef } from "react";

import { ResultCount } from "@/features/settings/components/result-count";
import { ScrollSections } from "@/features/settings/components/scroll-sections";
import { SearchResults } from "@/features/settings/components/search-results";
import { SectionNav } from "@/features/settings/components/section-nav";
import { SettingsSearch } from "@/features/settings/components/settings-search";
import { useSettingsSearch } from "@/features/settings/hooks/use-settings-search";
import type { Section, SectionGroup } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";

/**
 * Where a section of Settings is picked, under the field that finds settings and, at the Hub, the
 * Server picker in `children`: a list down the side, or on a phone a strip of them under the title,
 * which its arrows scroll. While the field has text, what it found takes their place, and on a phone
 * the pane's too.
 */
export function SettingsSide({
  groups,
  section,
  user,
  children,
}: {
  groups: SectionGroup[];
  section: Section;
  user: User | undefined;
  children?: ReactNode;
}) {
  const strip = useRef<HTMLElement>(null);
  const listId = useId();
  const search = useSettingsSearch(groups, user);
  const searching = search.typed !== "";
  const sections = (
    // Wider, the sections sit straight in the column, as the arrows are only a phone's.
    <div className="flex min-w-0 items-center gap-0.5 px-1 md:contents">
      <ScrollSections strip={strip} to="left" />
      <SectionNav ref={strip} groups={groups} section={section} />
      <ScrollSections strip={strip} to="right" />
    </div>
  );
  return (
    // The pane, its sibling, hides on a phone while this is `data-searching` (`settings-pane.tsx`).
    <div
      data-searching={searching || undefined}
      className="flex shrink-0 flex-col max-md:data-searching:min-h-0 max-md:data-searching:flex-1 md:w-48 md:gap-2.5 md:overflow-y-auto md:bg-sidebar md:px-2 md:py-3 lg:w-52"
    >
      {children}
      <SettingsSearch search={search} listId={listId} />
      {searching ? <SearchResults search={search} listId={listId} /> : sections}
      <ResultCount count={searching ? search.results.length : undefined} />
    </div>
  );
}
