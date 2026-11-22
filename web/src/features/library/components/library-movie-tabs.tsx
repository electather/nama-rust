import { Tabs, TabsPanel } from "nama-ui/components/tabs";

import { LibraryBoxSets } from "@/features/library/components/library-box-sets";
import { LibraryMovies } from "@/features/library/components/library-movies";
import { LibraryTabList } from "@/features/library/components/library-tab-list";
import { type LibraryView, viewFromSearch } from "@/features/library/lib/library-view";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/** A Movies Library's Movies and its Box Sets, a tab each, `view` the one shown. */
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
      <LibraryTabList />
      <TabsPanel value="movies">
        <LibraryMovies {...titles} />
      </TabsPanel>
      <TabsPanel value="box-sets">
        <LibraryBoxSets libraryId={titles.libraryId} />
      </TabsPanel>
    </Tabs>
  );
}
