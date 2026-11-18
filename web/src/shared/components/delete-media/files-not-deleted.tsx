import type { FileNotDeleted } from "@/gen/nama/v1/library_pb";

/** What a delete couldn't delete: how many files, then each with why. */
export function FilesNotDeleted({ files }: { files: FileNotDeleted[] }) {
  return (
    <>
      <p className="text-destructive">
        {files.length === 1 ? "A file" : `${files.length} files`} couldn’t be deleted. The rest
        went.
      </p>
      <ul className="mt-1 flex flex-col gap-1 text-muted-foreground">
        {files.map((file, index) => (
          <li key={`${String(index)} ${file.fileName}`} className="break-all">
            {file.fileName}: {file.reason}
          </li>
        ))}
      </ul>
    </>
  );
}
