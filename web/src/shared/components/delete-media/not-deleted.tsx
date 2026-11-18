import type { FileNotDeleted } from "@/gen/nama/v1/library_pb";
import { FilesNotDeleted } from "@/shared/components/delete-media/files-not-deleted";

/** The files the last delete couldn't delete, each with why. */
export function NotDeleted({ files }: { files: FileNotDeleted[] }) {
  return (
    <div role="alert" className="basis-full text-sm empty:hidden">
      {files.length > 0 && <FilesNotDeleted files={files} />}
    </div>
  );
}
