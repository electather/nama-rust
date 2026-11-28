import { LibraryBrowse } from "@/features/library/components/library-browse";
import { LibraryCountLine } from "@/features/library/components/library-count-line";
import { LibraryHeader } from "@/features/library/components/library-header";
import type { LibraryView } from "@/features/library/lib/library-view";
import type { TitleListing } from "@/features/library/lib/title-listing";
import { LibraryKind, type Library } from "@/gen/nama/v1/library_pb";
import { MayDownloadAgain } from "@/shared/components/delete-media/may-download-again";
import { useLibraries } from "@/shared/hooks/use-libraries";
import { backdropOf } from "@/shared/lib/server-image";

/**
 * A Library: its name and how many titles it lists, on its Backdrop if it has one, then the bar to
 * sort and filter them, a Movies Library's Box Sets in a tab beside its Movies, their grid, which
 * ends with its Unmatched Items, and a delete's notice.
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
  // The route's copy is the loader's, which doesn't hear Settings change it, such as its Backdrop.
  const live = useLibraries().find(({ id }) => id === library.id) ?? library;
  const titles = { libraryId: library.id, listing, change };
  return (
    <>
      <LibraryHeader
        name={live.name}
        backdrop={backdropOf(live)}
        count={<LibraryCountLine library={library} listing={listing} view={view} />}
      />
      <main className="mx-auto flex w-full max-w-350 flex-col gap-7.5 px-5 pt-7.5 pb-16">
        {deleted !== undefined && <MayDownloadAgain title={deleted} fulfillers={notTold} />}
        <LibraryBrowse
          shows={library.kind === LibraryKind.SHOWS}
          view={view}
          switchView={switchView}
          titles={titles}
        />
      </main>
    </>
  );
}
