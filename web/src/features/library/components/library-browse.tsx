import { LibraryMovieTabs } from "@/features/library/components/library-movie-tabs";
import { LibraryShows } from "@/features/library/components/library-shows";
import { TitleBar } from "@/features/library/components/title-bar";
import type { LibraryView } from "@/features/library/lib/library-view";
import type { TitlesProps } from "@/features/library/lib/titles-props";

/** A Library's bar and grid: a Shows Library's Shows, or a Movies Library's Movies and Box Sets tabs. */
export function LibraryBrowse({
  shows,
  view,
  switchView,
  titles,
}: {
  shows: boolean;
  view: LibraryView;
  switchView: (view: LibraryView) => void;
  titles: TitlesProps;
}) {
  if (!shows) {
    return <LibraryMovieTabs view={view} switchView={switchView} titles={titles} />;
  }
  return (
    <>
      <TitleBar titles={{ ...titles, kind: "shows" }} />
      <LibraryShows {...titles} />
    </>
  );
}
