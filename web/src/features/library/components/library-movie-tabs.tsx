import { Tabs } from "nama-ui/components/tabs";

import { LibraryTabList } from "@/features/library/components/library-tab-list";
import { LibraryTabPanels } from "@/features/library/components/library-tab-panels";
import { TitleBar } from "@/features/library/components/title-bar";
import { type LibraryView, viewFromSearch } from "@/features/library/lib/library-view";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/**
 * A Movies Library's Movies and its Box Sets, a tab each, `view` the one shown, the tabs in the
 * row with the Movies' sort and Filter.
 */
export function LibraryMovieTabs({
  view,
  switchView,
  titles,
}: {
  view: LibraryView;
  switchView: (view: LibraryView) => void;
  titles: TitlesProps;
}) {
  return (
    <Tabs
      value={view}
      onValueChange={(value) => {
        switchView(viewFromSearch({ view: value }).view ?? "movies");
      }}
    >
      <TitleBar
        start={<LibraryTabList />}
        titles={view === "movies" ? { ...titles, kind: "movies" } : undefined}
      />
      <LibraryTabPanels titles={titles} />
    </Tabs>
  );
}
