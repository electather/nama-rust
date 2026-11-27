import { BoxSetCountLine } from "@/features/library/components/box-set-count-line";
import { TitleCountLine } from "@/features/library/components/title-count-line";
import type { LibraryView } from "@/features/library/lib/library-view";
import type { TitleListing } from "@/features/library/lib/title-listing";
import { LibraryKind, type Library } from "@/gen/nama/v1/library_pb";

/** How many titles `library`'s header says it lists: its Box Sets on that tab, else as `listing` keeps them. */
export function LibraryCountLine({
  library,
  listing,
  view,
}: {
  library: Library;
  listing: TitleListing;
  view: LibraryView;
}) {
  if (library.kind === LibraryKind.SHOWS) {
    return <TitleCountLine libraryId={library.id} listing={listing} kind="shows" />;
  }
  return view === "box-sets" ? (
    <BoxSetCountLine libraryId={library.id} />
  ) : (
    <TitleCountLine libraryId={library.id} listing={listing} kind="movies" />
  );
}
