import { EmptyFolder } from "@/features/admin-libraries/components/empty-folder";
import type { Library } from "@/gen/nama/v1/library_pb";

/**
 * A warning for each of `library`'s folders that held files and read empty at its last scan, as an
 * unmounted drive does, with a way to remove what's missing from it once the Admin knows it's gone.
 */
export function EmptyFolders({ library }: { library: Library }) {
  if (library.emptyFolders.length === 0) {
    return null;
  }
  return (
    <ul aria-label="Folders that look empty" className="flex max-w-xl flex-col gap-3">
      {library.emptyFolders.map((folder) => (
        <EmptyFolder key={folder} libraryId={library.id} folder={folder} />
      ))}
    </ul>
  );
}
