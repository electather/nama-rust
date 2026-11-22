import { EmptyFolders } from "@/features/admin-libraries/components/empty-folders";
import { ScanButton } from "@/features/admin-libraries/components/scan-button";
import { ScanStatus } from "@/features/admin-libraries/components/scan-status";
import type { Library } from "@/gen/nama/v1/library_pb";

/**
 * Where `library`'s files are, a warning for each folder that looks empty, and how its scan is
 * going beside Scan, which also asks for one once a change to the folders is `foldersSaved`.
 */
export function LibraryScan({
  library,
  foldersSaved,
}: {
  library: Library;
  foldersSaved: boolean;
}) {
  const status = (
    <div className="flex min-w-0 flex-1 basis-50 flex-col gap-1.5">
      <ScanStatus libraryId={library.id} lastScan={library.lastScan} idle="No scan seen yet." />
      {foldersSaved && (
        <p className="text-sm text-pretty">Folders saved. Scan the Library to pick them up.</p>
      )}
    </div>
  );
  return (
    <div className="flex flex-col gap-4">
      <ul aria-label="Folders" className="flex flex-col gap-1 text-sm wrap-anywhere">
        {library.folders.map((folder) => (
          <li key={folder}>{folder}</li>
        ))}
      </ul>
      <EmptyFolders library={library} />
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
        {status}
        <ScanButton library={library} />
      </div>
    </div>
  );
}
