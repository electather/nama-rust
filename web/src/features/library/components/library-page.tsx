import { LibraryMovieTabs } from "@/features/library/components/library-movie-tabs";
import { LibraryShows } from "@/features/library/components/library-shows";
import { NotMatched } from "@/features/library/components/not-matched";
import type { LibraryView } from "@/features/library/lib/library-view";
import type { TitleListing } from "@/features/library/lib/title-listing";
import { LibraryKind, type Library } from "@/gen/nama/v1/library_pb";
import { MayDownloadAgain } from "@/shared/components/delete-media/may-download-again";
import { Page } from "@/shared/components/page";

/**
 * A Library: its titles, their sort and filter bar, a Movies Library's Box Sets in a tab beside
 * its Movies, its Unmatched Items and a delete's notice.
 */
export function LibraryPage({
  library,
  listing,
  view,
  deleted,
  notTold,
  change,
  switchView,
}: {
  library: Library;
  listing: TitleListing;
  view: LibraryView;
  deleted: string | undefined;
  notTold: string[];
  change: (listing: TitleListing) => void;
  switchView: (view: LibraryView) => void;
}) {
  const titles = { libraryId: library.id, listing, change };
  return (
    <Page heading={library.name}>
      {deleted !== undefined && (
        <div className="mt-2">
          <MayDownloadAgain title={deleted} fulfillers={notTold} />
        </div>
      )}
      {library.kind === LibraryKind.SHOWS ? (
        <LibraryShows {...titles} />
      ) : (
        <LibraryMovieTabs view={view} switchView={switchView} titles={titles} />
      )}
      <NotMatched libraryId={library.id} />
    </Page>
  );
}
