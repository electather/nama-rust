import { type ReactNode, useId } from "react";

import { ResultCount } from "@/features/settings/components/result-count";
import { SearchResults } from "@/features/settings/components/search-results";
import { SectionNav } from "@/features/settings/components/section-nav";
import { SettingsSearch } from "@/features/settings/components/settings-search";
import { useSettingsSearch } from "@/features/settings/hooks/use-settings-search";
import type { SectionGroup } from "@/features/settings/lib/sections";
import type { User } from "@/gen/nama/v1/sign_in_pb";
import type { Section } from "@/shared/lib/settings-address";

/**
 * Where a section of Settings is picked, under the field that finds settings and, at the Hub, the
 * Server picker in `children`: a list down the side, or on a phone the whole first page, as a
 * phone's settings are. While the field has text, what it found takes their place.
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
  const listId = useId();
  const search = useSettingsSearch(groups, user);
  const searching = search.typed !== "";
  const sections = <SectionNav groups={groups} section={section} />;
  return (
    <div
      data-searching={searching || undefined}
      className="flex min-h-0 flex-col max-md:flex-1 max-md:pt-1 md:w-48 md:shrink-0 md:gap-2.5 md:bg-sidebar md:px-2 md:py-3 lg:w-52"
    >
      {children}
      <SettingsSearch search={search} listId={listId} />
      {/* Only what's under the field scrolls, so Search stays at the top. */}
      <div className="flex min-h-0 flex-1 scrollbar-quiet flex-col overflow-y-auto overscroll-contain max-md:px-2 max-md:pb-4 md:-mx-2 md:gap-2.5 md:px-2">
        {searching ? <SearchResults search={search} listId={listId} /> : sections}
        <ResultCount count={searching ? search.results.length : undefined} />
      </div>
    </div>
  );
}
