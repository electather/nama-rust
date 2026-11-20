import { notFound } from "@tanstack/react-router";

import { DeleteLibrary } from "@/features/admin-libraries/components/delete-library";
import { EditLibrary } from "@/features/admin-libraries/components/edit-library";
import { EmptyFolders } from "@/features/admin-libraries/components/empty-folders";
import { HomePageImage } from "@/features/admin-libraries/components/home-page-image";
import { ScanButton } from "@/features/admin-libraries/components/scan-button";
import { ScanStatus } from "@/features/admin-libraries/components/scan-status";
import { useLibrary } from "@/features/admin-libraries/data/use-library";
import { Page } from "@/shared/components/page";
import { kindName } from "@/shared/lib/library-kind";

/** The Library `id`'s page: its folders, scan and Backdrop, and the form that changes it. */
export function LibraryPage({ id }: { id: bigint }) {
  const library = useLibrary(id);
  if (!library) {
    throw notFound();
  }
  return (
    <Page heading={library.name}>
      <p className="mt-1 text-sm text-muted-foreground">{kindName(library.kind)}</p>
      <ul aria-label="Folders" className="mt-4 flex flex-col gap-1 text-sm wrap-anywhere">
        {library.folders.map((folder) => (
          <li key={folder}>{folder}</li>
        ))}
      </ul>
      <EmptyFolders library={library} />
      {/* Pulled back by the buttons' own padding, so the first one's words line up with the name. */}
      <div className="-ms-4 mt-6 flex flex-wrap items-center gap-1">
        <ScanButton library={library} />
        <DeleteLibrary library={library} />
      </div>
      <div className="mt-2 max-w-sm">
        <ScanStatus libraryId={library.id} lastScan={library.lastScan} idle="No scan seen yet." />
      </div>
      <HomePageImage library={library} />
      <section aria-labelledby="edit" className="mt-10 max-w-sm">
        <h2 id="edit" className="font-medium">
          Name, folders and title sources
        </h2>
        <EditLibrary library={library} />
      </section>
    </Page>
  );
}
