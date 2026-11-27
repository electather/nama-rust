import { TabsPanel } from "nama-ui/components/tabs";

import { LibraryBoxSets } from "@/features/library/components/library-box-sets";
import { LibraryMovies } from "@/features/library/components/library-movies";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/** A Movies Library's tabs' panels, under their row: its Movies' grid, and its Box Sets'. */
export function LibraryTabPanels({ titles }: { titles: TitlesProps }) {
  return (
    <div className="mt-7.5">
      <TabsPanel value="movies">
        <LibraryMovies {...titles} />
      </TabsPanel>
      <TabsPanel value="box-sets">
        <LibraryBoxSets libraryId={titles.libraryId} />
      </TabsPanel>
    </div>
  );
}
